# envgg

Run commands with environment variables from `.env` files, with secrets resolved from your system keyring. The `.env` files only contain names, so they never hold secret values.

## Usage

```bash
envgg -- npm start                # loads .env
envgg development -- npm start    # loads .env.development
envgg p -- tsx src/index.ts       # loads .env.production
envgg --env-file .my-env -- npm start # loads a specific env file

envgg secrets # list the secrets stored in the keyring
envgg open    # open the GUI manager
envgg vars    # print the variable names used by the .env files in this folder
envgg export  # write all secrets as plaintext to .env.bak (-f to overwrite)
```

The environment is `development`, `staging`, `production`, `test` or `local` (or `d`, `s`, `p`, `t`, `l` for short), given as the first argument. To load any other file, pass its path with `--env-file`. The command to run goes after `--`. It replaces envgg, so its signals and exit code are its own. If it can't be started, envgg exits with 127 (not found) or 126 (not executable), and 125 if envgg itself failed. `envgg run p -- npm start` is the same as `envgg p -- npm start`.

See [CLI.md](docs/CLI.md) for the full command reference.

---

#### Env file format

```bash
# comment - will be ignored
FOO=123    [will be exported]
APP_SECRET [will be sourced from device keyring]
APP_SECRET=$ALIAS [ALIAS will be sourced from device keyring, and exported as APP_SECRET]
```

Quoted values can span multiple lines:

```bash
PRIVATE_KEY="-----BEGIN KEY-----
...
-----END KEY-----"
```
