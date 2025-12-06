# envgg

```
Run commands with environment variables from .env, .env.development, .env.staging, or .env.production

Usage: envgg [OPTIONS] [ARGS]...

Arguments:
  [ARGS]...  Arguments: [env] command...

             Where env is optional and can be: [d, development, s, staging, p, production]

             Examples:
             envgg npm start             # .env
             envgg development npm start # .env.development
             envgg d npm start           # .env.development
             envgg p tsx src/index.ts    # .env.production

Options:
  -l, --list     List all secrets stored in the `envgg` namespace in system keyring
  -o, --open     Open the GUI manager
  -c, --current  Print available environment variable names from suppported .env files in current folder
  -h, --help     Print help
```

---

#### Env file format

```bash
# comment - will be ignored
FOO=123    [will be exported]
APP_SECRET [will be sourced from device keyring]
APP_SECRET=$ALIAS [ALIAS will be sourced from device keyring, and exported as APP_SECRET]
```
