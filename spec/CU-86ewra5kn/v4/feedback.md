help me write a script in bash to ensure the user is
created and get an auth token, so we can use it as the
.env as CYAN_TOKEN.

the bash script will not need to check for binarie, most of these binaries exist
due to nix. add is as taskfile as e2e:setup (pls e2e:setup is for testing)

we can get a auth token with:

```
curl --request POST \
--url https://api.descope.com/v1/mgmt/tests/generate/otp \
--header 'authorization: Bearer {{DESCOPE_AUTH}}' \
--header 'content-type: application/json' \
--header 'cookie: ' \
--header 'x-descope-project-id: {{DESCOPE_PROJECT}}' \
--data '{
"loginId": "test1",
"deliveryMethod": "email"
}'
```

and

```
curl --request POST \
  --url https://api.descope.com/v1/auth/otp/verify/email \
  --header 'authorization: Bearer {{DESCOPE_AUTH}}' \
  --header 'content-type: application/json' \
  --data '{
  "loginId": "test1",
  "code": "{{OTP}}"
}'
```

to get the API's Bearer token, which is AUTH from here on

we can then check if our user is created with :

```
curl --request GET \
  --url http://localhost:9001/api/v1/User/{{USER_ID}} \
  --header 'authorization: Bearer {{AUTH}}'
```

if it doesn't exist, we can create:

```
  curl --request POST \
  --url http://localhost:9001/api/v1/User \
  --header 'authorization: Bearer {{AUTH}}' \
  --header 'content-type: application/json' \
  --data '{
  "username": "{{USER_NAME}}"
}'
```

We can then create a CYAN_TOKEN:

```
curl --request POST \ --url http://localhost:9001/api/v1/User/{{USER_ID}}/tokens \ --header 'authorization: Bearer {{AUTH}}' \ --header 'content-type: application/json' \ --data '{ "name": "<token name, doesn't matter>" }'
```

note that `{{DESCOPE_AUTH}}` is `{{DESCOPE_PROJECT}}:{{DESCOPE_TOKEN}}`

and they can be obtain via infisical (see how infisical script is executed). local is lapras and sulfone

these can be then generate the .env, which is currently:

```
CYANPRINT_USERNAME={{USER_NAME}}
CYANPRINT_REGISTRY=http://localhost:9001
CYANPRINT_COORDINATOR=http://localhost:9000
CYAN_TOKEN={{CYAN_TOKEN}}
DOCKER_USERNAME=
```

the user_id is `P2Wskb04HSJQRfckShfhtWXwUiUd` fixed, and we should pin the username `USER_NAME` to `cyane2e`

for docker username, we should just leave it blank, and let the user fill it up.

this should be e2e:setup script

lastly, for each folder in e2e that has a cyan.yaml inside (might be deep nested). set the username to `cyane2e` too
