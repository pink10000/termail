import smtplib
from email.mime import multipart, text, image


msg = multipart.MIMEMultipart()
msg.attach(text.MIMEText("This is a test email body."))
msg.attach(image.MIMEImage(open("test/assets/rust-1.png", "rb").read()))
msg["Subject"] = "Example: Test Image Attachment"
msg["From"] = "user1@example.com"
msg["To"] = "user1@example.com"

# Send via local GreenMail SMTP
with smtplib.SMTP("127.0.0.1", 1025) as server:
    server.send_message(msg)

print("Email sent successfully.")
