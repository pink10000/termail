# termail
terminal mail client inspired by mutt and notmuch

# testing
We use [Greenmail](https://github.com/greenmail-mail-test/greenmail) to test the application. You can run Greenmail by

```
docker compose -f test/docker-compose.yml up
```
or detach it with `-d`. 

You can send an email to the Greenmail server via 
```bash 
cargo run -- --cli send-email --to "user1@example.com"
```
The cli args `--to`, `--subject` and `--body` are optional but if they are provided they will be used to preffil the temp pop up editor. 

and test fetching the top email using
```bash 
cargo run -- --cli --backend Greenmail fetch-inbox
```

# acknowledgement

as part of UCSD's CSE 291Y