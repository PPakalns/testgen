use std::io::SeekFrom;
use std::path::Path;

use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;

pub async fn dos2unix_async<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let path = path.as_ref();

    println!("dos2unix async {}", path.to_string_lossy());

    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .await?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await?;

    let mut read = 0;
    let mut write = 0;

    while read < buf.len() {
        if read + 1 < buf.len() && buf[read] == b'\r' && buf[read + 1] == b'\n' {
            buf[write] = b'\n';
            read += 2;
            write += 1;
        } else {
            if write != read {
                buf[write] = buf[read];
            }
            read += 1;
            write += 1;
        }
    }

    // If pointers never diverged, nothing was replaced
    if write == buf.len() {
        return Ok(());
    }

    buf.truncate(write);

    file.seek(SeekFrom::Start(0)).await?;
    file.set_len(0).await?;
    file.write_all(&buf).await?;
    file.flush().await?;

    Ok(())
}

pub async fn dos2unix_command<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let mut s = tokio::process::Command::new("dos2unix");
    s.arg(path.as_ref());
    let s = s.status().await?;
    if !s.success() {
        anyhow::bail!("dos2unix failed: {}", s);
    }
    Ok(())
}
