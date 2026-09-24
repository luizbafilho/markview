# Launch and exit

A user runs `markview <file.md>`. The document fills the terminal with a status bar at the bottom. Pressing `q` or `Esc` returns them to the shell with the screen restored. A missing argument or unreadable file prints an error and exits 1 without taking over the screen.

## Sub-features

- `launch-open` opens the file and shows ` <file>  0%  ·  j/k ↑/↓ scroll · space/b page · g/G top/bottom · q quit`.
- `launch-quit-q` exits with code 0 on `q` and restores the terminal.
- `launch-quit-esc` exits with code 0 on `Esc`.
- `launch-usage` prints `Error: usage: markview <file.md>` and exits 1 when no file is given.
- `launch-missing` prints `Error: reading <path>` plus the OS error and exits 1 for a missing file.

## How to get to it (user POV)

- Run `markview notes.md` in kitty, Ghostty, or another terminal.
- Run `markview` with no argument, or with a path that does not exist.

## Driving it with mv-verify

Preconditions:

- `$V build` succeeded.

- **Open.** Run `$V launch launch-1 sample.md`. It prints `ready: run=launch-1 terminal=kitty`. Then `$V text launch-1 open` shows `Search v2 launch plan` near the top and ` sample.md  0%` on the last line.
- **Quit with q.** Run `$V kitty launch-1 send-text q`, then `$V wait launch-1 'markview exited 0'`. `$V text launch-1 after-quit` contains `markview exited 0` and no document text. `target/verify/launch-1/exit-code` holds `0`.
- **Quit with Esc.** Relaunch as `launch-2`, run `$V kitty launch-2 send-key escape`, then `$V wait launch-2 'markview exited 0'`.
- **Usage error.** Run `target/debug/markview </dev/null; echo "exit=$?"`. stderr is `Error: usage: markview <file.md>` and the last line is `exit=1`.
- **Missing file.** Run `target/debug/markview nope.md </dev/null; echo "exit=$?"`. stderr starts with `Error: reading nope.md`, then `Caused by:` and `No such file or directory (os error 2)`, and the last line is `exit=1`.
- **Proof.** Keep `open.txt`, `after-quit.txt`, `exit-code`, and the captured stderr of both error runs.

## Gotchas

- `launch` waits for the status bar. If it times out, the screen it last saw goes to stderr. A blank screen usually means markview is stuck waiting on a terminal reply (see the SKILL.md gotcha about bare PTYs).
- The error runs need no window. They fail before markview touches the terminal, so run them directly.
- `Esc` is sent with `send-key escape`, not `send-text`.
