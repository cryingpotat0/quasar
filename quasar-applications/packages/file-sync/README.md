```
# From one terminal
➜  projects npx @quasartc/file-sync --url quasar-connect.cryingpotato.com --user-type leader --directory ./src
info: 18-cherry-honeydew
info: Connected!

info: Sent add for: a
info: Sent change for: a
info: Sent change for: a
```


```
# From the other terminal
npx @quasartc/file-sync --url quasar-connect.cryingpotato.com --user-type follower --directory ./src --code 18-cherry-honeydew
```
