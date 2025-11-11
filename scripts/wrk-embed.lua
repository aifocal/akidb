-- wrk Lua script for testing /api/v1/embed endpoint
wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = '{"texts": ["The quick brown fox jumps over the lazy dog", "Machine learning is transforming software"]}'
