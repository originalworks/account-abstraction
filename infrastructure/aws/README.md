### How to deploy to AWS

## Prequisites

### Database

This template doesn't create a database for you. You need to create one and set the following parameters in `samconfig.toml`:

- `DatabaseAccessSecurityGroup` - security group ID that allows access to your database.
- `PrivateSubnets` - private subnets IDs that allow access to your database. One or more subnets can be specified, separated by commas.

**Important:** These are `AWS::SSM::Parameter::Value<String>` parameters type stored in AWS SSM Parameter Store.

**Note:** You can use simple String parameters instead of SSM parameters, just edit the `template.yaml` file and change the parameters type to `String`.

### Secrets

Template assume the following secrets are set in you AWS Secrets Manager:

For Database (pointed by 'DbSecretsName'):

- `password`
- `username`
- `host`
- `port`

For KMS master signing key (pointed by 'MasterKmsSecretsName'):

- `AA_MASTER_KMS_ID`

**Note:** You can use one secret for both

## Deployment steps:

1. Go to `./infrastructure`
2. `./build_workers.sh`
3. `sam deploy --config-env {dev|stage|prod}`
