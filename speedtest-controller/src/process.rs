use std::process::Stdio;

use futures::StreamExt;
use regex::Regex;
use tokio::{
    process::{Child, Command},
    select,
};
use tokio_util::codec::{FramedRead, LinesCodec};

/// Creates a process using the given `Command`, waits for a pattern to match in the process output,
/// and returns the transformed output and the child process.
///
/// # Arguments
///
/// * `c` - The `Command` used to create the process.
/// * `re` - The regular expression pattern to match in the process output.
/// * `transform` - A closure that transforms the captured groups from the pattern match into the desired output.
///
/// # Generic Parameters
///
/// * `const N: usize` - The number of captured groups expected from the pattern match.
/// * `T` - The type of the transformed output.
/// * `Output` - The type of the transformed output.
///
/// # Returns
///
/// Returns a tuple containing the transformed output and the child process.
///
/// # Panics
///
/// Panics if the process does not give any output that matches the specified regular expression pattern.
pub async fn create_process_and_wait_for_pattern<const N: usize, T, Output>(
    mut c: Command,
    re: Regex,
    transform: T,
) -> (Output, Child)
where
    T: FnOnce([&str; N]) -> Output,
{
    let mut process = c
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("failed to execute process");

    let mut stdout = FramedRead::new(process.stdout.take().unwrap(), LinesCodec::new())
        .map(|data| data.expect("fail on stdout!"));

    let mut stderr = FramedRead::new(process.stderr.take().unwrap(), LinesCodec::new())
        .map(|data| data.expect("fail on stderr!"));

    loop {
        let line = select! {
             Some(v) = stdout.next() => v,
             Some(v) = stderr.next() => v,
             else => break,
        };
        if let Some((_, group)) = re.captures_iter(&line).map(|c| c.extract()).next() {
            return (transform(group), process);
        }
    }

    panic!(
        "The process did not give any output that is accept by the regex {}",
        re
    )
}
