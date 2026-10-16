# wanshitong-api

The JSON types of WST's external HTTP API, shared by the server (`wanshitong-web`)
and its clients (`wst`).

They are the API's contract, kept apart from the domain types so that those can
change without changing the API. See `spec/api-conventions.md`.
