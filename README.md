# **DORE**: **DO**cker **R**emote **E**xecutor

Run `dore` inside docker compose project and it will upload docker configuration files to remote server and execute command inside remote host

## Usage examples

```
dore up service1 service2 # sync files and run service1 and service2
dore down -v service3 # shutdown service3 and drop volumes
```

## Tool logic

`dore` starts search fo `compose.yaml` and `docker-remote.yaml` from current folder and up above. If there is no `docker-remote.yaml` config file, program should work in _local_ mode, act as a proxy to the real `docker` installed on a local machine. In case where both files are found `dore` will work in _remote_ mode.

In _remote_ mode if there is a "gap" between folders of those files, same gap will be kept on the server. When configs found, `dore` will deserialize
- own config file `docker-remote.yaml` to know where to send files
- docker config file `compose.yaml` to find files that referenced by it

After this `dore` starts search for files on local computer with respect to `.dockerignore` and own config rules. When files collected they will be copied over SFTP. After this `dore` will execute command on the remote `docker` service

## Config file

To connect remote server create a file called `docker-remote.yaml`. File structure:
- `target` describes remote server to connect
    - `user` - ssh user name
    - `host` - remote server name or IP address
    - `port` - SSH tcp port
    - `path` - where to put files on remote machine. By default files are stored relatively to SSH user home directory. Use `/absolute/path/to/folder` to specify exact location
- `auth` describes how to login to SSH
    - `key` - path to SSH private key. Put "~/.ssh/id_rsa" here to use user-wide key or per-project key
    - `password` - put plain text password here to use password authentication
- `files`, optional section
    - `exclude` - similar to `.gitignore`: list files that should not be transferred to remote host
    - `include` - if some files not reference explicitly by docker, but required on remote host, this is the way how to transfer it

## Current limitations

Only `up`, `down` and `build` command supported for now
