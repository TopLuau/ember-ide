// Word Count - a minimal Ember plugin.

fn init() {
    register_command("word-count.count", "Word Count: Show statistics");
}

fn on_command(id) {
    let text = editor_text();
    let words = text.split().len();
    let lines = text.split("\n").len();
    let chars = text.len();
    notify(`${words} words, ${lines} lines, ${chars} characters`);
    log(`counted ${current_file()}`);
}
