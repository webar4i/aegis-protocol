//! Local development PKI only. Never upload its output or deploy this CA.
use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose,
};
use std::{error::Error, fs, path::Path};

fn write(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let destination = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "dev-pki".into());
    let destination = Path::new(&destination);
    // Refuse to replace existing credentials, including a partially created PKI.
    fs::create_dir(destination)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o700))?;
    }
    let ca_key = KeyPair::generate()?;
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let ca = params.self_signed(&ca_key)?;
    write(&destination.join("ca.pem"), ca.pem().as_bytes())?;
    for (name, san, usage) in [
        ("server", "localhost", ExtendedKeyUsagePurpose::ServerAuth),
        (
            "client",
            "aegis-device.local",
            ExtendedKeyUsagePurpose::ClientAuth,
        ),
    ] {
        let key = KeyPair::generate()?;
        let mut leaf = CertificateParams::new(vec![san.to_owned()])?;
        leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf.extended_key_usages = vec![usage];
        let certificate = leaf.signed_by(&key, &ca, &ca_key)?;
        write(
            &destination.join(format!("{name}.pem")),
            certificate.pem().as_bytes(),
        )?;
        write(
            &destination.join(format!("{name}.key")),
            key.serialize_pem().as_bytes(),
        )?;
    }
    // The signing key is intentionally not persisted; regenerate a new directory
    // for a new demo. On Windows protect this directory with your account ACL.
    println!("Created local-only test credentials. Keep the directory private.");
    Ok(())
}
