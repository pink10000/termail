import smtplib
from email.mime.text import MIMEText

msg = MIMEText("This is a test email body.")
msg["Subject"] = "Hello from Python AGAINNNNNNNNNNNN"
msg["From"] = "user1@example.com"
msg["To"] = "user1@example.com"

# Send via local GreenMail SMTP
with smtplib.SMTP("127.0.0.1", 1025) as server:
    server.send_message(msg)

print("Email sent successfully.")
