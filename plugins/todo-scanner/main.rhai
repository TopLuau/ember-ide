// TODO Scanner - shows how event hooks work.

fn init() {
    register_command("todo-scanner.list", "TODOs: List in current file");
}

fn scan() {
    let found = [];
    let n = 0;
    for line in editor_text().split("\n") {
        n += 1;
        if line.contains("TODO") || line.contains("FIXME") {
            found.push(`${n}: ${line.trim()}`);
        }
    }
    found
}

// Fired by the IDE whenever a file is opened.
fn on_open(path) {
    let found = scan();
    if found.len() > 0 {
        notify(`${found.len()} TODO/FIXME item(s) in this file`);
    }
}

fn on_command(id) {
    let found = scan();
    if found.len() == 0 {
        notify("No TODO or FIXME comments here");
    } else {
        for f in found { log(f); }
        notify(`${found.len()} item(s) - see Output panel`);
    }
}
