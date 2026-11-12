# Security

## Reporting a vulnerability

Report vulnerabilities through GitHub's private vulnerability reporting, not a public issue; the report stays private, and the maintainer acknowledges it within 7 days. Only the latest release and `develop` are supported.

- Path: Security tab > Report a vulnerability.

## Threat model

fennec has three trust zones.

1. The human is trusted. The human uses the CLI/UI and configuration, and fennec never blocks the human; the human's terminal, editor, servers, and own git run with normal rights.
2. The trusted kernel is trusted. It is the code that decides integration and plugin activation, never runs what a model wrote, and is the one git runner; the human's rules are kernel code too.
3. The untrusted zone is not trusted. It contains everything a model outputs, including tool calls, commands, and files it writes, so a prompt injection in a README, dependency, or web page can make a model run anything. Commands run in the sandbox unless the human turns it off, plugins run sandboxed, and the sandbox is the wall while rules are guard rails.

## What fennec protects

The table lists the main protections, each naming the claim, mechanism, and tests. These protections are local to the developer's machine and do not make model output trusted.

| Claim | How | Test(s) |
| --- | --- | --- |
| The server only answers local pages. | It checks `Host` and `Origin` for 127.0.0.1/localhost and requires JSON for POST. This guards against DNS rebinding and CSRF. | `crates/fennec/src/web/guard.rs::only_local_pages_may_post_json`; `crates/fennec/src/web/guard.rs::a_foreign_host_is_checked_before_the_content_type`; `crates/fennec/src/web/guard.rs::an_https_localhost_origin_is_refused_with_403` |
| Opt-in remote access needs a password and a session cookie. | Remote access is refused to start without a password hash. A remote caller needs its session cookie and an `Origin` equal to its `Host`. | `crates/fennec/src/domain/server_config.rs::listening_beyond_the_loopback_needs_a_password`; `crates/fennec/src/web/login.rs::an_api_call_without_a_cookie_is_refused`; `crates/fennec/src/web/login.rs::an_unknown_session_cookie_is_refused`; `crates/fennec/src/web/login.rs::a_valid_session_cookie_is_granted`; `crates/fennec/src/domain/password.rs::the_right_password_verifies`; `crates/fennec/src/domain/password.rs::the_wrong_password_does_not_verify`; `crates/fennec/src/web/guard.rs::remote_enabled_accepts_a_matching_origin` |
| A remote session ends with a password change and after 30 days. | Changing the password drops every session; the server forgets a session when its cookie expires. | `crates/fennec/src/web/login.rs::changing_the_password_ends_every_session`; `crates/fennec/src/web/login.rs::an_expired_session_is_refused` |
| A sandboxed command is never taken for the human. | With the network on, a sandboxed command shares the loopback. On Linux the server finds which process opened each local connection (`/proc`) and refuses a process inside a bwrap that fennec started; on macOS the Seatbelt profile denies fennec's own port. The caller then falls under the remote rule: refused, or asked for the password. | `crates/fennec/src/sandbox/peer.rs::e2e_a_sandboxed_client_is_recognised_and_the_test_process_is_not`; `crates/fennec/src/sandbox/seatbelt.rs::online_world_denies_the_server_port` |
| Commands run in a sandbox. | On Linux, bwrap runs commands with a read-only root, no network by default, hidden dirs such as `~/.ssh`, and hidden container sockets. On macOS, Seatbelt gives the same world. | `crates/fennec/src/sandbox.rs::e2e_the_root_filesystem_is_read_only`; `crates/fennec/src/sandbox.rs::e2e_without_network_only_lo_exists`; `crates/fennec/src/sandbox.rs::e2e_container_daemon_sockets_are_hidden`; `crates/fennec/src/sandbox.rs::e2e_a_hidden_dir_is_not_readable`; `crates/fennec/src/sandbox.rs::from_prefs_lists_only_paths_that_exist`; `crates/fennec/src/sandbox/seatbelt.rs::macos::write_outside_workdir_fails`; `crates/fennec/src/sandbox/seatbelt.rs::macos::hidden_dir_read_fails_writable_inside_succeeds`; `crates/fennec/src/sandbox/seatbelt.rs::macos::offline_network_fails` (the three Seatbelt tests: macOS only, run by hand before a release) |
| A read-only mode makes no remote call that could change something. | An HTTP MCP call bypasses the sandbox, so in a read-only mode only a tool the server declares read-only (`readOnlyHint`) runs; the others are refused. | `crates/fennec/src/tools.rs::an_http_mcp_call_is_refused_in_read_only_mode`; `crates/fennec/src/tools.rs::an_http_mcp_tool_declared_write_is_refused_in_read_only_mode`; `crates/fennec/src/tools.rs::an_http_mcp_tool_declared_read_only_is_not_refused_in_read_only_mode` |
| File tools are confined to the workdir. | The path guard denies traversal outside the workdir and symlink escapes. | `crates/fennec/src/tools/path_guard.rs::a_deep_traversal_outside_the_workdir_is_denied`; `crates/fennec/src/tools/path_guard.rs::symlink_escape_denied`; `crates/fennec/src/tools/path_guard.rs::a_symlinked_directory_pointing_outside_denies_files_under_it` |
| The file explorer stays inside the project and never overwrites unseen changes. | Paths are parsed without `..`, absolute paths or `.git`; symlinks and symlinked parents are refused; paths reach git literally (`--literal-pathspecs`); a save needs the blob id the human read (409 otherwise) and writes only that file, and a commit takes only the files the human ticks, each one a changed tracked file, and a reset takes one changed tracked file and never deletes an added one. A session's version is read from its branch through git, never from disk. | `crates/fennec/src/domain/project_file.rs::a_plain_nested_path_is_accepted`; `crates/fennec/src/domain/project_file.rs::traversal_absolute_and_git_paths_are_refused`; `crates/fennec/src/domain/project_file.rs::session_ids_with_slashes_dots_or_a_leading_dash_are_refused`; `crates/fennec/src/domain/project_file.rs::a_blank_message_is_refused_and_a_padded_one_is_trimmed`; `crates/fennec/src/project_files.rs::a_symlink_pointing_outside_the_project_is_refused`; `crates/fennec/src/project_files.rs::a_symlinked_directory_pointing_outside_is_refused`; `crates/fennec/src/project_files.rs::a_stale_base_is_refused_and_nothing_is_written`; `crates/fennec/src/project_files.rs::a_pathspec_magic_name_is_taken_literally`; `crates/fennec/src/project_files.rs::committing_a_file_that_did_not_change_is_refused`; `crates/fennec/src/project_files.rs::one_commit_takes_the_ticked_files_and_keeps_the_rest_staged`; `crates/fennec/src/project_files.rs::restoring_an_added_file_unstages_it_and_keeps_it_on_disk`; `crates/fennec/src/project_files.rs::restoring_a_file_that_did_not_change_is_refused` |
| Raw file bytes are served only for images, and an SVG runs no script. | `/raw` answers only image extensions (never html or js), with a sandbox `Content-Security-Policy` and `nosniff`; the editor shows them through `<img>`, where SVG scripts never run. | `crates/fennec/src/domain/project_file.rs::image_types_come_from_the_extension_and_others_are_none`; `crates/fennec/src/project_files.rs::a_file_that_is_not_an_image_is_not_read_raw`; `crates/fennec/src/project_files.rs::a_symlinked_image_pointing_outside_is_refused`; `crates/fennec/src/web/routes/file_routes.rs::raw_serves_an_svg_with_the_sandbox_csp_and_nosniff`; `crates/fennec/src/web/routes/preview.rs::preview_serves_html_with_the_sandbox_csp`; `crates/fennec/src/web/routes/canvas_routes.rs::canvas_content_serves_html_with_the_sandbox_csp` |
| Protected files are refused to file tools and read-only in the sandbox. | File tools cannot write protected paths. The sandbox makes protected paths read-only. | `crates/fennec/src/tools/path_guard.rs::writing_to_protected_paths_is_refused`; `crates/fennec/src/sandbox.rs::e2e_read_only_paths_are_read_only_in_the_workdir` |
| The workdir's `.git` is read-only. | File tools cannot write git metadata paths. The sandbox makes git metadata read-only. | `crates/fennec/src/tools/path_guard.rs::git_metadata_paths_are_refused_for_writing`; `crates/fennec/src/sandbox.rs::e2e_git_metadata_is_read_only_in_the_workdir` |
| Kernel block rules stop `git push` and branch moves. | The kernel refuses `git push`, `update-ref`, `branch -D`, `worktree`, and `gh pr merge` before they run. | `crates/fennec/src/domain/kernel_rules.rs::blocks_push_variants`; `crates/fennec/src/domain/kernel_rules.rs::blocks_branch_and_ref_changes`; `crates/fennec/src/domain/kernel_rules.rs::allows_read_only_and_local_git_commands` |
| Kernel git never runs repository hooks or fsmonitor. | Every kernel git command sets `core.hooksPath=/dev/null` and `core.fsmonitor=false`. | `crates/fennec/src/git.rs::host_git_does_not_run_model_written_hooks_or_fsmonitor` |
| Integration is a human decision behind a review gate. | The review gate blocks on protected files, suppressions, and weakened tests. The human decides integration. | `crates/fennec/src/domain/integration.rs::architecture_violations_block_after_protected_files`; `crates/fennec/src/domain/integration.rs::suppressions_and_weakened_tests_block_after_architecture`; `crates/fennec/src/domain/bypass.rs::suppressions_flag_the_marker_families`; `crates/fennec/src/domain/bypass.rs::an_assertion_removed_with_code_added_elsewhere_blocks` |
| Night mode never lets a model answer for the human. | With the human away, the kernel answers a workflow's human step with its forward choice and records it as a decision to review; a task naming a risky action is not started. Integration still waits for the human. | `crates/fennec/src/agent/workflow/tests.rs::night_mode_answers_a_human_node_with_its_first_choice`; `crates/fennec/src/agent/workflow/tests.rs::night_mode_validates_the_tests_like_a_click`; `crates/backlog/src/risk.rs::delete_tests_and_publish` |
| The human's rules decide with block over allow over ask. | The rule engine applies block, then allow, then ask. | `crates/rules/src/decide.rs::block_beats_allow_beats_ask`; `crates/rules/src/decide.rs::allow_beats_ask` |
| Plugins require the sandbox. | A plugin is not active without the sandbox. | `crates/fennec/src/plugins/host.rs::plugin_requires_sandbox`; `crates/fennec/src/plugins/host.rs::plugin_sandbox` |
| Stored API keys are sealed at rest with `secret.key`. | The config loads or creates a master key at the `secret.key` path. | `crates/fennec/src/config.rs::a_sealed_key_opens_with_the_master_key`; `crates/fennec/src/web/routes/settings_routes.rs::a_plain_key_is_stored_sealed` (the sealing itself is tested in llm-providers-connector's `secret.rs`) |

## What fennec does NOT protect

### Commands and the sandbox

- Command rules match parsed tokens, so `eval`, a variable, or a script can slip past them.
- The sandbox does not read the rules: a path rule guards the file tools only, and a command can still write a blocked path inside the work folder.
- A session's worktree shares its refs with the human's repository.
- Night mode spots a risky task by phrase matching on its text, plan and sheet: a task that words a risky action otherwise is started, and runs in the sandbox like any other.
- With the network on, `git push` reaches the remote, and the kernel refuses `git push` only by token matching.
- The human can turn the sandbox off.
- The rest of the home directory stays readable to commands, except the secrets fennec hides; `~/.cargo/registry`, `~/.cargo/git`, `~/.cache`, and `~/.npm` stay writable, and a command can leave a tampered file there that a later build reuses.

### File tools and protected files

- The file tools run in fennec's process with a working-directory path guard.
- A command can create a new file matching a protected pattern where none existed.
- Renaming a protected file's parent directory is not refused below the workdir's top level on bwrap, not refused at all on Seatbelt, and a protected file below the top of the folder can be replaced by renaming its parent folder.
- The read-only mount only holds at the top level for `.agents`, `.github`, and a root lockfile.
- A `.git` created by a command where none existed is not read-only.
- Its config can name filter, textconv, or merge drivers that a kernel git command in that directory would run.

### Network and fennec's own server

- `web_fetch` and `web_search` run in fennec's process, not in the sandbox, and they reach the web even with the network off.
- `web_fetch` checks a host's address, then connects: a DNS name that changes its answer in between (rebinding) can reach a private address such as another local service or a cloud metadata endpoint, for a GET.
- Remote access is plain HTTP: on a network you do not trust, the password and the session cookie can be read in transit. Put fennec behind a TLS proxy or a VPN.
- A reverse proxy on the same machine that rewrites `Host` to `127.0.0.1` makes every remote caller look local, with no login. Keep the original `Host`.
- The servers the play button starts run outside the sandbox.
- In `just dev`, Vite's proxy on `:5173` forwards `/api` to fennec: a sandboxed command with the network on reaches fennec through it, as Vite. The released binary has no such proxy.
- A command can reach the internet when the network is on and could send what it reads in the project elsewhere.
- In a read-only mode, an HTTP MCP tool runs on the server's word: a server that declares a writing tool read-only makes a remote change from that mode.

### Configuration and the model

- Losing `secret.key` loses the stored keys.
- fennec trusts the model endpoint the human configures.
- Prompt injection in files the agent reads is contained, not prevented.

The review gate is the last line before integration, and the human decides.

## Running the checks yourself

`just check` runs `web` (the UI build), `lint` (`cargo fmt --check`, `cargo clippy -D warnings`, and `web/scripts/check-ui.mjs`, which fails when a component restyles a UI primitive by hand), `test` (`cargo test`), `test-web` (Vitest), and `licenses` (`scripts/licenses.py check`, which refuses a dependency whose licence the AGPL cannot absorb). `just test` runs all Rust tests, including the sandbox e2e tests, in a private TMPDIR; those sandbox e2e tests need a working bwrap, and on Ubuntu 24.04 AppArmor may block it, with the Settings page showing the command to allow it. CI runs the same recipes on every pull request.
