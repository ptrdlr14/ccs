# ccs

Claude Code Profile Switcher.  
Launch Claude Code with named profiles for arguments and environment variables.

## Install

Recommend via mise:
```bash
mise use -g github:ptrdlr14/ccs@latest
```

Or via cargo-binstall:
```bash
cargo binstall --git https://github.com/ptrdlr14/ccs.git ccs
```

Or via cargo:
```bash
cargo install --git https://github.com/ptrdlr14/ccs.git
```

## Usage

```bash
ccs                         # show usage and available profiles
ccs --help                  # show help (-h)
ccs --version               # show ccs version (-V)
ccs <profile>               # launch with a profile
ccs <profile> -p "hello"     # append arguments for this launch
ccs <profile> --help         # forward --help to Claude Code
```

All arguments after the profile name are preserved and appended after the
profile's generated arguments, including `--help`, `--version`, and `--`.
Invalid options and unknown profiles print an error with usage and exit with code 2.

## Configuration

Create, edit, and delete profiles in `~/.config/ccs/config.toml`. If the file does
not exist, ccs creates an empty configuration with a commented example when
listing or launching profiles.

```toml
[shared.common.args]
permission-mode = "plan"
plugin-dir = ["./plugins/tools"]

[shared.common.env]
CLAUDE_CODE_MAX_OUTPUT_TOKENS = "16000"

[shared.proxy.env]
ANTHROPIC_BASE_URL = "https://api.example.com"
ANTHROPIC_AUTH_TOKEN = { from_env = "MY_API_KEY" }
ANTHROPIC_API_KEY = ""

[profiles.official]
description = "Official account"

[profiles.coding]
description = "Coding"
uses = ["common", "proxy"]

[profiles.coding.args]
permission-mode = "auto"

[profiles.coding.env]
ANTHROPIC_MODEL = "coding-model"
```

Each profile accepts four optional fields:

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `description` | String | Omitted | Description displayed in the profile list |
| `uses` | Array of strings | `[]` | Shared configurations applied in order |
| `args` | Table | Empty | Named command-line options |
| `env` | Table | Empty | Environment variables applied to the child process |

Shared configurations contain `args` and `env`. They are referenced by `uses`,
do not appear in the profile list, and cannot be launched directly. Shared
configurations do not reference other shared configurations or profiles.

Unknown configuration fields and invalid value types are rejected.
Configuration errors exit with code 1. Help and version commands do not read
the configuration.

### Shared configuration

ccs applies each name in `uses` from left to right, then the profile's own
`args` and `env`. Both tables merge by key: later values replace earlier values.
Arrays are replaced as a whole, preserving their element order and duplicates.
Entries that are not overridden remain in the result.

For `coding` above, `common` supplies the plugin directory and output limit,
`proxy` supplies the endpoint and credentials, and `coding` replaces
`permission-mode` and adds its model. Missing shared configurations are reported
when launching the profile that references them.

### Arguments

Use long option names without the `--` prefix:

| Value | Example | Generated arguments |
|-------|---------|---------------------|
| String | `permission-mode = "plan"` | `--permission-mode=plan` |
| Boolean | `verbose = true` | `--verbose` |
| Array of strings | `plugin-dir = ["./a", "./b"]` | `--plugin-dir=./a --plugin-dir=./b` |

`false` and empty arrays generate no arguments and can replace inherited values.
An empty string generates `--option=`. Arguments are generated in option-name
order; array elements retain their configured order. Positional arguments such
as prompts can be passed on the command line.

Running `ccs coding -p "Review this project"` with the example above launches
the equivalent of:

```bash
claude --permission-mode=auto --plugin-dir=./plugins/tools -p "Review this project"
```

ccs passes values directly without invoking a shell. Spaces are preserved;
variables, `~`, and wildcards are not expanded. It does not maintain a list of
Claude Code options or supply their defaults. Claude Code interprets the options
and their precedence relative to environment variables. Command-line arguments
are appended unchanged; duplicate options between the configuration and command
line are handled by Claude Code.

### Environment variables

Each entry is either a literal string or a reference to a variable in the
environment that launched ccs:

```toml
[profiles.example.env]
LITERAL = "${HOME}"                       # passed literally
EMPTY = ""                               # set to an empty string
TOKEN = { from_env = "MY_API_KEY" }       # read MY_API_KEY at launch
```

References are resolved after all shared and profile entries are merged.
Overridden references are not read. References always read the original parent
environment, including variables that ccs removes or overrides for the child;
they do not reference other entries in `env`. A missing source variable stops
the launch with an error identifying the profile, destination variable, and
source variable. An empty source value is valid. Listing profiles does not
resolve references.

Before applying the merged environment, ccs removes inherited `ANTHROPIC_*`
variables. All other shell variables, including `CLAUDE_CODE_*`, are inherited
unless overridden by `env`. An empty profile clears inherited `ANTHROPIC_*`
variables and adds no arguments or environment variables of its own.

`KEY = ""` sets an empty string; it does not remove the variable. Authentication
variables are configured explicitly, including `ANTHROPIC_API_KEY = ""` in the
proxy example.

## Reference

See [the Claude Code configuration reference](docs/claude-code.md) for the
checked Claude Code version, model and authentication environment variables,
startup arguments, and a versioned CLI help snapshot.

## License

MIT
