use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};

fn verify() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let [artifact, signature] = arguments.as_slice() else {
        return Err("usage: verify-update-artifact ARTIFACT SIGNATURE".into());
    };
    let public_key = std::env::var("GDOM_UPDATER_PUBLIC_KEY")?;
    let public_key = STANDARD.decode(public_key.trim())?;
    let public_key = PublicKey::decode(std::str::from_utf8(&public_key)?)?;
    let signature = std::fs::read_to_string(signature)?;
    let signature = STANDARD.decode(signature.trim())?;
    let signature = Signature::decode(std::str::from_utf8(&signature)?)?;
    public_key.verify(&std::fs::read(artifact)?, &signature, true)?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    match verify() {
        Ok(()) => {
            println!("Updater artifact signature matches the embedded public key.");
            std::process::ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!(
                "Updater artifact verification failed. Check the artifact, signature and public key."
            );
            std::process::ExitCode::FAILURE
        }
    }
}
