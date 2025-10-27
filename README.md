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
python3 test/test_send_mail.py
```

and test fetching the top email using
```bash 
cargo run -- --cli --backend Greenmail fetch-inbox
```

# acknowledgement

as part of UCSD's CSE 291Y