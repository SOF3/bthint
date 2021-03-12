use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use futures::future;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout_at;

use crate::Result;

type TestUnitResult = Result<Option<String>>;

pub async fn verify_php(string: &str) -> Option<String> {
    if !string.contains(|c| c == ';' || c == '$' || c == '{') {
        return None;
    }

    let lines: Vec<_> = string.split('\n').collect();
    for start in 0..(lines.len().saturating_sub(5)) {
        for end in ((start + 5)..lines.len()).rev() {
            let mut blocks: Vec<Pin<Box<dyn Future<Output = TestUnitResult> + Send>>> = Vec::new();
            for &class in &[false, true] {
                let mut buf = if lines[start].starts_with("<?php") {
                    String::new()
                } else {
                    String::from("<?php\n")
                };
                if class {
                    buf += "class Foo {";
                }
                for line in &lines[start..end] {
                    buf += line;
                    buf += "\n";
                }
                if class {
                    buf += "}";
                }

                let lines = &lines;
                let block = async move {
                    log::debug!("Piping lines {}..{} {} to php -l", start, end, &buf);
                    let mut cmd = Command::new("php")
                        .arg("-l")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .kill_on_drop(true)
                        .spawn()?;

                    log::debug!("Writing stdin");
                    {
                        let mut stdin = cmd
                            .stdin
                            .take()
                            .ok_or_else(|| anyhow!("Failed to open stdin"))?;
                        stdin.write_all(buf.as_bytes()).await?;
                    }

                    log::debug!("Waiting for php -l completion");
                    let exit = cmd.wait().await?;
                    log::debug!("php -l returned {:?}", exit);
                    if exit.success() {
                        Ok(Some(lines[start..end].join("\n")))
                    } else {
                        Ok(None)
                    }
                };
                blocks
                    .push(Box::pin(block) as Pin<Box<dyn Future<Output = TestUnitResult> + Send>>);
            }
            let timeout = Instant::now() + Duration::from_secs(5);
            while !blocks.is_empty() {
                let all_futures = future::select_all(blocks);
                let (resolve, _, rest) = match timeout_at(timeout.into(), all_futures).await {
                    Ok(select) => select,
                    Err(_) => return None,
                };
                if let Ok(Some(resolve)) = resolve {
                    return Some(resolve);
                }
                blocks = rest;
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::verify_php;

    #[tokio::test]
    pub async fn test_stmt() {
        let _ = pretty_env_logger::try_init();

        let result = verify_php(
            r#"
            some random stuff
            hello();
            $world = foo();
            bar($baz); // has semicolon here
            $qux = 1;
            $corge += $qux;
            more random stuff
        "#,
        )
        .await;
        assert!(result.is_some());
        let result = verify_php(
            r#"
            some random stuff
            hello();
            $world = foo();
            bar($baz) // missing semicolon here
            $qux = 1;
            $corge += $qux;
            more random stuff
        "#,
        )
        .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    pub async fn test_fn() {
        let _ = pretty_env_logger::try_init();

        let result = verify_php(
            r#"
            public function foo() {
                some random stuff
                hello();
                $world = foo();
                bar($baz); // has semicolon here
                $qux = 1;
                $corge += $qux;
                more random stuff
            }
        "#,
        )
        .await;
        assert!(result.is_some());
        let result = verify_php(
            r#"
            public function foo() {
                sone random stuff
                hello();
                $world = foo();
                bar($baz) // missing semicolon here
                $qux = 1;
                $corge += $qux;
                more random stuff
            }
        "#,
        )
        .await;
        assert!(result.is_none());
    }
}
