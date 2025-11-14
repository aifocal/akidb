#!/bin/bash
# Send onboarding emails via Mailgun API
# Usage: ./send-onboarding-email.sh <email_type> <recipient_email> <user_data_json>

set -euo pipefail

# Configuration
MAILGUN_API_KEY="${MAILGUN_API_KEY:-your-mailgun-api-key}"
MAILGUN_DOMAIN="${MAILGUN_DOMAIN:-mg.akidb.com}"
FROM_EMAIL="welcome@akidb.com"
FROM_NAME="AkiDB Team"

# Email type (welcome, reminder, tips, upgrade, winback)
EMAIL_TYPE="$1"
RECIPIENT_EMAIL="$2"
USER_DATA="${3:-{}}"

# Extract user data
FIRST_NAME=$(echo "$USER_DATA" | jq -r '.first_name // "there"')
API_KEY=$(echo "$USER_DATA" | jq -r '.api_key // "YOUR_API_KEY"')
QUOTA_USED=$(echo "$USER_DATA" | jq -r '.quota_used // 0')

send_welcome_email() {
    local subject="Welcome to AkiDB! Your API key is ready 🚀"
    local html_body="<!DOCTYPE html>
<html>
<head>
    <meta charset='UTF-8'>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; }
        .container { max-width: 600px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 30px; border-radius: 8px 8px 0 0; }
        .content { background: #f9f9f9; padding: 30px; border-radius: 0 0 8px 8px; }
        .code-block { background: #1e1e1e; color: #d4d4d4; padding: 15px; border-radius: 4px; overflow-x: auto; font-family: 'Courier New', monospace; font-size: 14px; }
        .button { display: inline-block; background: #667eea; color: white; padding: 12px 24px; text-decoration: none; border-radius: 4px; margin: 10px 0; }
        .footer { text-align: center; color: #666; font-size: 12px; margin-top: 30px; }
    </style>
</head>
<body>
    <div class='container'>
        <div class='header'>
            <h1>Welcome to AkiDB!</h1>
            <p>Your account is ready. Let's get you started in 5 minutes.</p>
        </div>
        <div class='content'>
            <h2>Hi ${FIRST_NAME},</h2>

            <p>Welcome to AkiDB! Your account is ready and waiting.</p>

            <h3>Your API Key:</h3>
            <div class='code-block'>${API_KEY}</div>

            <h3>Get started in 5 minutes:</h3>

            <p><strong>1. Install the SDK:</strong></p>
            <div class='code-block'>
pip install akidb  # Python
npm install @akidb/client  # JavaScript
            </div>

            <p><strong>2. Create your first collection:</strong></p>
            <div class='code-block'>
import akidb

client = akidb.Client(api_key=\"${API_KEY}\")
collection = client.create_collection(
    name=\"my-first-collection\",
    dimension=384,
    metric=\"cosine\"
)
            </div>

            <p><strong>3. Insert vectors:</strong></p>
            <div class='code-block'>
collection.insert([
    {\"vector\": [0.1, 0.2, ...], \"metadata\": {\"text\": \"Hello world\"}}
])
            </div>

            <p><strong>4. Search:</strong></p>
            <div class='code-block'>
results = collection.search([0.1, 0.2, ...], top_k=10)
            </div>

            <a href='https://docs.akidb.com/quickstart' class='button'>View Full Quickstart</a>

            <h3>Need help?</h3>
            <ul>
                <li>📚 <a href='https://docs.akidb.com'>Documentation</a></li>
                <li>💬 <a href='https://discord.gg/akidb'>Discord Community</a></li>
                <li>📧 Email: support@akidb.com</li>
            </ul>

            <p>Happy building!</p>
            <p><strong>The AkiDB Team</strong></p>

            <p style='margin-top: 30px; padding-top: 20px; border-top: 1px solid #ddd; color: #666; font-size: 14px;'>
                P.S. You're on the Free tier (1M vectors, 100 QPS). Need more? <a href='https://akidb.com/pricing'>Upgrade anytime</a>.
            </p>
        </div>
        <div class='footer'>
            <p>AkiDB | Production-ready vector search with 99.99% SLA</p>
            <p><a href='https://akidb.com'>Website</a> | <a href='https://docs.akidb.com'>Docs</a> | <a href='https://github.com/akidb/akidb'>GitHub</a></p>
        </div>
    </div>
</body>
</html>"

    curl -s --user "api:${MAILGUN_API_KEY}" \
        "https://api.mailgun.net/v3/${MAILGUN_DOMAIN}/messages" \
        -F from="${FROM_NAME} <${FROM_EMAIL}>" \
        -F to="${RECIPIENT_EMAIL}" \
        -F subject="${subject}" \
        -F html="${html_body}" \
        -F "o:tracking=yes" \
        -F "o:tracking-clicks=yes" \
        -F "o:tracking-opens=yes"
}

send_reminder_email() {
    local subject="Quick question - stuck on anything?"
    local html_body="<!DOCTYPE html>
<html>
<body style='font-family: sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;'>
    <h2>Hi ${FIRST_NAME},</h2>

    <p>I noticed you haven't created your first collection yet. Is everything okay?</p>

    <p>Common issues we see:</p>
    <ul>
        <li>❓ Not sure which embedding model to use → Try <code>sentence-transformers/all-MiniLM-L6-v2</code> (384-dim)</li>
        <li>❓ Confused about dimensions → Must match your embedding model output</li>
        <li>❓ API errors → Check your API key is correct</li>
    </ul>

    <p><strong>Need a hand?</strong></p>
    <p>Reply to this email or book a 15-minute onboarding call: <a href='https://calendly.com/akidb/onboarding'>Schedule Call</a></p>

    <p>We're here to help!</p>

    <p>Best,<br>
    <strong>[Your Name]</strong><br>
    Founder, AkiDB</p>
</body>
</html>"

    curl -s --user "api:${MAILGUN_API_KEY}" \
        "https://api.mailgun.net/v3/${MAILGUN_DOMAIN}/messages" \
        -F from="${FROM_NAME} <${FROM_EMAIL}>" \
        -F to="${RECIPIENT_EMAIL}" \
        -F subject="${subject}" \
        -F html="${html_body}"
}

send_tips_email() {
    local subject="3 tips to get the most out of AkiDB"
    local html_body="<!DOCTYPE html>
<html>
<body style='font-family: sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;'>
    <h2>Hi ${FIRST_NAME},</h2>

    <p>Great job creating your first collection! 🎉</p>

    <p>Here are 3 tips to level up:</p>

    <h3>1. Optimize search quality</h3>
    <ul>
        <li>Use <code>top_k=20</code> then re-rank with a cross-encoder</li>
        <li>Set <code>include_metadata=True</code> to debug relevance</li>
        <li>Experiment with metrics: cosine (default), dot, l2</li>
    </ul>

    <h3>2. Batch insert for speed</h3>
    <ul>
        <li>Insert 100-1,000 docs per request (not one-by-one)</li>
        <li>Use async client for >10 concurrent operations</li>
        <li>Example: <code>collection.insert([doc1, doc2, ..., doc1000])</code></li>
    </ul>

    <h3>3. Monitor performance</h3>
    <ul>
        <li>Check collection stats: <code>collection.stats()</code></li>
        <li>Set up alerts for high latency</li>
        <li>Use our Grafana dashboard: <a href='https://grafana.akidb.com'>grafana.akidb.com</a></li>
    </ul>

    <p><strong>Building something cool?</strong><br>
    We'd love to feature your project in our showcase! Reply with details.</p>

    <p>Best,<br>
    <strong>[Your Name]</strong><br>
    Founder, AkiDB</p>
</body>
</html>"

    curl -s --user "api:${MAILGUN_API_KEY}" \
        "https://api.mailgun.net/v3/${MAILGUN_DOMAIN}/messages" \
        -F from="${FROM_NAME} <${FROM_EMAIL}>" \
        -F to="${RECIPIENT_EMAIL}" \
        -F subject="${subject}" \
        -F html="${html_body}"
}

send_upgrade_email() {
    local subject="You're ${QUOTA_USED}% through your Free tier quota"
    local quota_pct="${QUOTA_USED}"

    local html_body="<!DOCTYPE html>
<html>
<body style='font-family: sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;'>
    <h2>Hi ${FIRST_NAME},</h2>

    <p>Heads up - you've used ${quota_pct}% of your Free tier quota.</p>

    <h3>What happens at 100%?</h3>
    <ul>
        <li>Inserts will fail with \"quota exceeded\" error</li>
        <li>Existing data remains searchable</li>
        <li>You'll need to upgrade or delete vectors</li>
    </ul>

    <h3>Upgrade to Startup tier?</h3>
    <ul>
        <li>10M vectors (10x more space)</li>
        <li>1,000 QPS (10x higher throughput)</li>
        <li>99.9% SLA (vs 99% on Free)</li>
        <li>Email support (24h response)</li>
        <li><strong>\$499/month (50% off first 3 months with code LAUNCH50)</strong></li>
    </ul>

    <p>
        <a href='https://akidb.com/upgrade?code=LAUNCH50' style='display: inline-block; background: #667eea; color: white; padding: 12px 24px; text-decoration: none; border-radius: 4px;'>Upgrade Now</a>
    </p>

    <p>Or schedule a call to discuss: <a href='https://calendly.com/akidb/upgrade'>Book Call</a></p>

    <p>Thanks for using AkiDB!</p>

    <p>Best,<br>
    <strong>[Your Name]</strong><br>
    Founder, AkiDB</p>

    <p style='margin-top: 30px; padding-top: 20px; border-top: 1px solid #ddd; color: #666; font-size: 14px;'>
        P.S. Need more time to decide? Let us know - we can extend your Free tier temporarily.
    </p>
</body>
</html>"

    curl -s --user "api:${MAILGUN_API_KEY}" \
        "https://api.mailgun.net/v3/${MAILGUN_DOMAIN}/messages" \
        -F from="${FROM_NAME} <${FROM_EMAIL}>" \
        -F to="${RECIPIENT_EMAIL}" \
        -F subject="${subject}" \
        -F html="${html_body}"
}

# Main execution
case "$EMAIL_TYPE" in
    welcome)
        echo "Sending welcome email to ${RECIPIENT_EMAIL}..."
        send_welcome_email
        ;;
    reminder)
        echo "Sending reminder email to ${RECIPIENT_EMAIL}..."
        send_reminder_email
        ;;
    tips)
        echo "Sending tips email to ${RECIPIENT_EMAIL}..."
        send_tips_email
        ;;
    upgrade)
        echo "Sending upgrade email to ${RECIPIENT_EMAIL}..."
        send_upgrade_email
        ;;
    *)
        echo "Unknown email type: ${EMAIL_TYPE}"
        echo "Usage: $0 <welcome|reminder|tips|upgrade> <email> <user_data_json>"
        exit 1
        ;;
esac

echo "Email sent successfully!"
