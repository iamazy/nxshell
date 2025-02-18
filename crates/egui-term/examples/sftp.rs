use std::io::Read;

use egui_term::TermError;
use tracing::{error, trace};
use wezterm_ssh::{Config, Session, SessionEvent};

fn main() -> Result<(), TermError> {
    let mut config = Config::new();
    config.add_default_config_files();
    let config = config.for_host("127.0.0.1");
    smol::block_on(async move {
        let (session, events) = Session::connect(config)?;

        while let Ok(event) = events.recv().await {
            match event {
                SessionEvent::Banner(banner) => {
                    if let Some(banner) = banner {
                        trace!("{}", banner);
                    }
                }
                SessionEvent::HostVerify(verify) => {
                    verify.answer(true).await?;
                }
                SessionEvent::Authenticate(auth) => {
                    // login with ssh config, so no answers needed
                    auth.answer(vec![]).await?;
                }
                SessionEvent::HostVerificationFailed(failed) => {
                    error!("host verification failed: {failed}");
                    return Err(TermError::HostVerification(failed));
                }
                SessionEvent::Error(err) => {
                    error!("ssh login error: {err}");
                    return Err(TermError::Box(err.into()));
                }
                SessionEvent::Authenticated => break,
            }
        }

        let mut exec_ret = session.exec("pwd", None).await.unwrap();

        let mut s = String::new();
        exec_ret.stdout.read_to_string(&mut s).unwrap();

        let sftp = session.sftp();
        match sftp.read_dir(s.trim()).await {
            Ok(entries) => {
                for (path, _) in entries {
                    println!("path: {}", path.as_path())
                }
            }
            Err(err) => println!("{err}"),
        }

        Ok(())
    })?;

    Ok(())
}
