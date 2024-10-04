use std::io::Read;

use ring::digest::{Context, Digest, SHA256};

pub fn sha256_digest<R>(reader: &mut R) -> Result<Digest, std::io::Error>
where
    R: Read,
{
    let mut ctx = Context::new(&SHA256);
    let mut buffer = [0; 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        ctx.update(&buffer[..count]);
    }

    Ok(ctx.finish())
}
