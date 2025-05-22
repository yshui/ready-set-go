Ready, Set, Go!
===============

Simple utility to give you a countdown before running a command. Useful for preventing running unwanted commands by mistake.

Build
-----

```bash
rustc rsg.rs
```

Usage
-----

```bash
rsg [options] <command> [args...]

Options:

    -t TIMEOUT  Set the countdown timer [default: 5s]
```

Alternatively, create a symbolic link to `rsg`. When `rsg` is executed via a symlink, it will execute the command same as the name of the symlink. For example:

```bash
ln -s rsg shutdown
./shutdown # will start a countdown for shutting down
```

`rsg` cannot take command line options in this form, so to specify a timeout, set the `RSG_TIMEOUT` environment variable. For command specific timeouts, set `RSG_<COMMAND>_TIMEOUT`. The precedences of these are:

`-t` > `RSG_<COMMAND>_TIMEOUT` > `RSG_TIMEOUT`.
