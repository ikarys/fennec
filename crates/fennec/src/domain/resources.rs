//! The figures fennec's own process uses to answer a plugin's resource
//! request: pure parsing and computation, no I/O.

use serde::Serialize;

/// One reading of fennec's own process: CPU time and resident memory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessSample {
    pub cpu_ticks: u64,
    pub rss_bytes: u64,
    pub at_ms: u64,
}

/// The figures a plugin gets.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceFigures {
    pub rss_bytes: u64,
    pub cpu_percent: Option<f64>,
}

/// /proc reports CPU times in USER_HZ, fixed at 100 by the kernel ABI on the
/// architectures fennec targets.
pub const USER_HZ: u64 = 100;

/// The CPU ticks in a `/proc/self/stat` line.
///
/// The command name (field 2) is in parentheses and may contain spaces and
/// `)`; split after the last `)`. The fields after it start at field 3
/// (state), so `utime` is the 12th field after the split and `stime` the
/// 13th.
pub fn parse_cpu_ticks(stat: &str) -> Option<u64> {
    let (_, after) = stat.rsplit_once(")")?;
    let mut fields = after.split_whitespace().skip(11);
    let utime = fields.next()?.parse::<u64>().ok()?;
    let stime = fields.next()?.parse::<u64>().ok()?;
    utime.checked_add(stime)
}

/// The resident memory in a `/proc/self/status` line.
///
/// The `VmRSS:` line holds kilobytes; the result is bytes.
pub fn parse_rss_bytes(status: &str) -> Option<u64> {
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        let kibibytes = rest.split_whitespace().next()?.parse::<u64>().ok()?;
        return kibibytes.checked_mul(1024);
    }
    None
}

/// The figures a plugin gets from one current sample and the previous one.
///
/// `cpu_percent` is relative to one core, like `top`, rounded to one decimal.
/// It is `None` when there is no previous sample, when the time delta is zero,
/// or when the CPU ticks went backwards.
pub fn figures(previous: Option<&ProcessSample>, current: &ProcessSample) -> ResourceFigures {
    let mut figures = ResourceFigures {
        rss_bytes: current.rss_bytes,
        cpu_percent: None,
    };
    let Some(previous) = previous else {
        return figures;
    };
    let Some(delta_ms) = current.at_ms.checked_sub(previous.at_ms) else {
        return figures;
    };
    if delta_ms == 0 {
        return figures;
    }
    let Some(delta_ticks) = current.cpu_ticks.checked_sub(previous.cpu_ticks) else {
        return figures;
    };
    let seconds = delta_ms as f64 / 1000.0;
    let ticks = delta_ticks as f64 / USER_HZ as f64;
    let percent = ticks / seconds * 100.0;
    figures.cpu_percent = Some((percent * 10.0).round() / 10.0);
    figures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cpu_ticks_handles_a_command_name_with_parens() {
        // Fields after the last `)`: state, ppid, pgrp, session, tty_nr,
        // tpgid, flags, minflt, cminflt, majflt, cmajflt, utime, stime.
        let stat = "42 (my ) proc) S 0 1 2 3 4 5 6 7 8 9 50 25";
        assert_eq!(parse_cpu_ticks(stat), Some(75));
    }

    #[test]
    fn parse_cpu_ticks_rejects_a_line_without_a_closing_paren() {
        assert_eq!(parse_cpu_ticks("42 S 0 1 2 3 4 5 6 7 8 9 50 25"), None);
    }

    #[test]
    fn parse_rss_bytes_reads_the_vm_rss_line() {
        let status = "Name:\tfennec\nVmRSS:\t  12345 kB\nThreads:\t1\n";
        assert_eq!(parse_rss_bytes(status), Some(12345 * 1024));
    }

    #[test]
    fn figures_has_no_cpu_percent_without_a_previous_sample() {
        let current = ProcessSample {
            cpu_ticks: 50,
            rss_bytes: 4096,
            at_ms: 1000,
        };
        assert_eq!(
            figures(None, &current),
            ResourceFigures {
                rss_bytes: 4096,
                cpu_percent: None,
            }
        );
    }

    #[test]
    fn figures_turns_50_ticks_in_one_second_into_50_percent() {
        let previous = ProcessSample {
            cpu_ticks: 0,
            rss_bytes: 4096,
            at_ms: 0,
        };
        let current = ProcessSample {
            cpu_ticks: 50,
            rss_bytes: 8192,
            at_ms: 1000,
        };
        assert_eq!(
            figures(Some(&previous), &current),
            ResourceFigures {
                rss_bytes: 8192,
                cpu_percent: Some(50.0),
            }
        );
    }

    #[test]
    fn figures_has_no_cpu_percent_for_a_zero_time_delta() {
        let previous = ProcessSample {
            cpu_ticks: 0,
            rss_bytes: 4096,
            at_ms: 1000,
        };
        let current = ProcessSample {
            cpu_ticks: 50,
            rss_bytes: 8192,
            at_ms: 1000,
        };
        assert_eq!(
            figures(Some(&previous), &current),
            ResourceFigures {
                rss_bytes: 8192,
                cpu_percent: None,
            }
        );
    }

    #[test]
    fn figures_has_no_cpu_percent_when_ticks_go_backwards() {
        let previous = ProcessSample {
            cpu_ticks: 50,
            rss_bytes: 4096,
            at_ms: 0,
        };
        let current = ProcessSample {
            cpu_ticks: 10,
            rss_bytes: 8192,
            at_ms: 1000,
        };
        assert_eq!(
            figures(Some(&previous), &current),
            ResourceFigures {
                rss_bytes: 8192,
                cpu_percent: None,
            }
        );
    }
}
