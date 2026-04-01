use std::io::Write;
use std::process::Child;

pub struct BackgroundJob {
    pub id: usize,
    #[allow(dead_code)]
    pub pid: u32,
    pub command: String,
    pub child: Child,
}

pub struct JobManager {
    jobs: Vec<BackgroundJob>,
}

impl JobManager {
    #[must_use]
    pub fn new() -> Self {
        Self { jobs: Vec::new() }
    }

    #[allow(clippy::maybe_infinite_iter)]
    fn next_id(&self) -> usize {
        (1..).find(|n| !self.jobs.iter().any(|j| j.id == *n)).unwrap()
    }

    /// Add a background job. Prints `[id] pid` to stdout.
    pub fn add(&mut self, child: Child, command: String) {
        let id = self.next_id();
        let pid = child.id();
        println!("[{id}] {pid}");
        self.jobs.push(BackgroundJob {
            id,
            pid,
            command,
            child,
        });
    }

    /// Check all jobs; print "Done" for finished ones and remove them.
    pub fn reap(&mut self) {
        let len = self.jobs.len();
        let done_indices: Vec<usize> = self
            .jobs
            .iter_mut()
            .enumerate()
            .filter_map(|(i, job)| {
                if matches!(job.child.try_wait(), Ok(Some(_))) {
                    let marker = if i + 1 == len {
                        '+'
                    } else if i + 2 == len {
                        '-'
                    } else {
                        ' '
                    };
                    println!("[{}]{}  {:<24}{}", job.id, marker, "Done", job.command);
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        for i in done_indices.into_iter().rev() {
            self.jobs.remove(i);
        }
    }

    /// Print all jobs (Running/Done) to `out`. Reaps Done jobs after listing.
    pub fn list_jobs(&mut self, out: &mut dyn Write) {
        let len = self.jobs.len();
        let mut done_indices = Vec::new();
        for (i, job) in self.jobs.iter_mut().enumerate() {
            let is_done = matches!(job.child.try_wait(), Ok(Some(_)));
            let status = if is_done { "Done" } else { "Running" };
            let marker = if i + 1 == len {
                '+'
            } else if i + 2 == len {
                '-'
            } else {
                ' '
            };
            if is_done {
                let _ = writeln!(out, "[{}]{}  {:<24}{}", job.id, marker, status, job.command);
                done_indices.push(i);
            } else {
                let _ = writeln!(out, "[{}]{}  {:<24}{} &", job.id, marker, status, job.command);
            }
        }
        for i in done_indices.into_iter().rev() {
            self.jobs.remove(i);
        }
    }

    /// Wait for all remaining background jobs (called at REPL exit).
    pub fn wait_all(&mut self) {
        for job in &mut self.jobs {
            let _ = job.child.wait();
        }
        self.jobs.clear();
    }
}
