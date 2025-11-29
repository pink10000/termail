import smtplib
from email.mime.text import MIMEText

body = "".join([f"{i} text\n" for i in range(100)])

msg = MIMEText(body)
msg["Subject"] = "Example: Many Lines"
msg["From"] = "user1@example.com"
msg["To"] = "user1@example.com"

# Send via local GreenMail SMTP
with smtplib.SMTP("127.0.0.1", 1025) as server:
    server.send_message(msg)

print("Email sent successfully.")
