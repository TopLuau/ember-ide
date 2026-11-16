# Writing Ember plugins

A plugin is a folder containing `plugin.toml` and `main.rhai`:

```
my-plugin/
  plugin.toml
  main.rhai
```

```toml
name = "My Plugin"
version = "1.0.0"
author = "You"
description = "What it does."
```

Plugins are loaded from `<config dir>/ember/plugins`, `plugins/` next to the executable, and the platform's shared data dir. Enable or disable them in the **Plugins** side panel.

## Hooks

Define any of these functions in `main.rhai`:

| Function | Called |
| --- | --- |
| `init()` | once at load — register commands here |
| `on_command(id)` | when one of your commands runs from the palette |
| `on_open(path)` | after a file is opened |
| `on_before_save(path)` | just before saving |
| `on_save(path)` | after saving |

## Host API

`notify(msg)`, `log(msg)`, `editor_text()`, `set_editor_text(text)`, `current_file()`, `project_root()`, `open_file(path)`, `register_command(id, title)`, `now()`, `rtrim(s)`, `join(array, sep)`.

## Sandboxing

Scripts run in a restricted Rhai engine: no file or network access beyond the host API, an operation budget (runaway loops are stopped), and expression depth limits.

## Example

```rhai
fn init() {
    register_command("hello.say", "Hello: Say hi");
}

fn on_command(id) {
    if id == "hello.say" {
        notify("Hello from " + current_file());
    }
}
```
