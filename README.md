# envgg

```
Run commands with environment variables from .env, .env.development, .env.staging, or .env.production

Usage: envgg <ARGS>...

Arguments:
  <ARGS>...  Arguments: [env] command...

             Where env is optional and can be: d, development, s, staging, p, production

             Examples:
             envgg npm start          # .env
             envgg d npm start        # .env.development
             envgg p tsx src/index.ts # .env.production

Options:
  -h, --help  Print help
```

---

#### Env file format

```bash
# comment - will be ignored
FOO=123    [will be exported]
APP_SECRET [will be sourced from device keyring]
```
