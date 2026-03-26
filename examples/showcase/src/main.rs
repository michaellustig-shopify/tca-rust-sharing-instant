//! Showcase launcher — press a key to run any recipe example.

use std::io::Read;
use std::process::Command;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const MAGENTA: &str = "\x1b[35m";
const RED: &str = "\x1b[31m";
const BLUE: &str = "\x1b[34m";

struct Recipe {
    key: char,
    name: &'static str,
    package: &'static str,
    desc: &'static str,
    color: &'static str,
    api: &'static str,
}

const RECIPES: &[Recipe] = &[
    Recipe {
        key: '1',
        name: "Avatar Stack",
        package: "avatar-stack",
        desc: "Who's online — presence with status",
        color: GREEN,
        api: "Room<UserPresence>",
    },
    Recipe {
        key: '2',
        name: "Todos",
        package: "todos",
        desc: "Collaborative todo list — CRUD + reactive sync",
        color: CYAN,
        api: "subscribe + transact",
    },
    Recipe {
        key: '3',
        name: "Reactions",
        package: "reactions",
        desc: "Emoji broadcast — pub/sub messaging",
        color: YELLOW,
        api: "TopicChannel<EmojiReaction>",
    },
    Recipe {
        key: '4',
        name: "Typing Indicator",
        package: "typing-indicator",
        desc: "Who's typing — presence + idle timeout",
        color: MAGENTA,
        api: "Room<TypingPresence>",
    },
    Recipe {
        key: '5',
        name: "Cursors",
        package: "cursors",
        desc: "Random-walk cursors on ASCII grid",
        color: BLUE,
        api: "Room<CursorPresence>",
    },
    Recipe {
        key: '6',
        name: "Merge Tiles",
        package: "merge-tiles",
        desc: "Collaborative 4x4 colored grid",
        color: RED,
        api: "subscribe + transact",
    },
];

fn enable_raw_mode() -> libc::termios {
    unsafe {
        let mut orig: libc::termios = std::mem::zeroed();
        libc::tcgetattr(libc::STDIN_FILENO, &mut orig);
        let mut raw = orig;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw);
        orig
    }
}

fn restore_terminal(orig: &libc::termios) {
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, orig);
    }
}

fn print_menu() {
    print!("\x1b[2J\x1b[H");
    println!();
    println!("  {BOLD}sharing-instant recipes{RESET}");
    println!("  {DIM}Press a number to launch. Press q to quit.{RESET}");
    println!();

    for r in RECIPES {
        println!(
            "  {BOLD}{}{RESET}  {}{}  {RESET}{DIM}{}{RESET}",
            r.key, r.color, r.name, r.desc
        );
        println!("      {DIM}API: {}{RESET}", r.api);
        println!();
    }

    println!("  {BOLD}q{RESET}  {DIM}Quit{RESET}");
    println!();
}

fn main() {
    let orig = enable_raw_mode();
    let mut stdin = std::io::stdin().lock();
    let mut buf = [0u8; 1];

    loop {
        print_menu();

        match stdin.read(&mut buf) {
            Ok(1) => match buf[0] {
                b'q' | b'Q' | 3 => break,
                key => {
                    if let Some(recipe) = RECIPES.iter().find(|r| r.key as u8 == key) {
                        restore_terminal(&orig);
                        println!(
                            "  {BOLD}Launching {}{}{RESET}...\n",
                            recipe.color, recipe.name
                        );

                        let status = Command::new("cargo")
                            .args(["run", "-p", recipe.package])
                            .status();

                        match status {
                            Ok(s) if s.success() => {}
                            Ok(s) => {
                                eprintln!(
                                    "\n  {DIM}{} exited with {}{RESET}",
                                    recipe.name, s
                                );
                            }
                            Err(e) => {
                                eprintln!("\n  {DIM}Failed to launch: {e}{RESET}");
                            }
                        }

                        println!("\n  {DIM}Press any key to return to menu...{RESET}");
                        let orig2 = enable_raw_mode();
                        let _ = stdin.read(&mut buf);
                        restore_terminal(&orig2);
                        let _ = enable_raw_mode();
                    }
                }
            },
            _ => break,
        }
    }

    restore_terminal(&orig);
    println!("\n  Goodbye!\n");
}
