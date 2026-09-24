pipeline {
    agent any

    environment {
        DROPLET_HOST = '165.22.225.235'
        DROPLET_USER = 'jenkins'
        DEPLOY_PATH = '/home/jenkins/nb-blog'
    }

    stages {
        stage('Prepare') {
            steps {
                echo 'loading env file...'
                withCredentials([file(credentialsId: 'nb-blog-env-file', variable: 'ENV_FILE')]) {
                    sh 'cp -f -- "$ENV_FILE" .env'
                    sh 'ls -la .env'
                }
                echo 'loaded env file....'
            }
        }

        stage('Build') {
            steps {
                sh 'docker build -t novabyte-api:latest .'
                sh 'docker save -o nb-api_docker-image.tar novabyte-api:latest'
                sh 'rm -f nb-api_docker-image.tar.xz'
                sh 'xz -T0 -9 nb-api_docker-image.tar'
            }
        }

        // ONE-TIME. Prod has never talked to SurrealKit — it has no
        // __entity/__rollout tracking tables yet, just the pre-finance
        // tables. This tells SurrealKit "the schema files present right now
        // already exist live, start tracking from here" without running any
        // DDL. Must run with database/schema/ containing ONLY the
        // pre-finance files (person/post/token/meta/functions) — if the
        // finance files were present, SurrealKit would wrongly mark those
        // tables as already-tracked and the real migration below would
        // compute a zero diff and never create them.
        //
        // DELETE THIS STAGE after running it once against prod.
        // stage('Bootstrap Prod Baseline') {
        //     steps {
        //         withCredentials([file(credentialsId: 'nb-blog-surrealkit-env', variable: 'SURREALKIT_ENV_FILE')]) {
        //             sh '''
        //                 cp -f "$SURREALKIT_ENV_FILE" .env.local
        //                 mkdir -p /tmp/finance-schema-holding
        //                 mv database/schema/income_record.surql database/schema/expense_record.surql \
        //                    database/schema/tax_payment.surql database/schema/tax_profile.surql \
        //                    database/schema/tax_rules.surql /tmp/finance-schema-holding/
        //                 surrealkit rollout baseline -v
        //                 mv /tmp/finance-schema-holding/*.surql database/schema/
        //             '''
        //         }
        //     }
        // }

        // Applies any rollout manifest(s) under database/rollouts/ that prod
        // hasn't completed yet — authored and reviewed locally beforehand
        // via `cargo make db-plan`, never generated in CI. Runs before
        // Deploy so the app is never started against a schema it doesn't
        // expect yet. Connects straight to wss://db.novabyte.blog (nginx
        // proxying to the now-loopback-only SurrealDB port) — no SSH
        // involved, since this now runs directly on the Jenkins agent
        // rather than via SSH-exec on the droplet.
        //
        // Requires `surrealkit` on the agent's PATH (see Dockerfile.agent)
        // and the same nb-blog-surrealkit-env credential as the bootstrap
        // stage above.
        stage('Migrate DB') {
            steps {
                withCredentials([file(credentialsId: 'nb-blog-surrealkit-env', variable: 'SURREALKIT_ENV_FILE')]) {
                    sh '''
                        cp -f "$SURREALKIT_ENV_FILE" .env.local
                        chmod +x scripts/migrate-db.sh
                        ./scripts/migrate-db.sh
                    '''
                }
            }
        }

        stage('Deploy') {
            steps {
                withCredentials([sshUserPrivateKey(
                    credentialsId: 'nb-blog_droplet-deploy-key',
                    keyFileVariable: 'PK'
                )]) {
                    sh '''
                        ssh-keyscan -H ${DROPLET_HOST} >> ~/.ssh/known_hosts
                        ssh -i "$PK" ${DROPLET_USER}@${DROPLET_HOST} "rm -f ${DEPLOY_PATH}/.env ${DEPLOY_PATH}/nb-api_docker-image.tar.xz ${DEPLOY_PATH}/nb-api_docker-image.tar"
                        scp -i "$PK" nb-api_docker-image.tar.xz ${DROPLET_USER}@${DROPLET_HOST}:${DEPLOY_PATH}/
                        scp -i "$PK" .env ${DROPLET_USER}@${DROPLET_HOST}:${DEPLOY_PATH}/
                        ssh -i "$PK" ${DROPLET_USER}@${DROPLET_HOST} "cd ${DEPLOY_PATH} && xz -d nb-api_docker-image.tar.xz && docker load -i nb-api_docker-image.tar"
                        ssh -i "$PK" ${DROPLET_USER}@${DROPLET_HOST} "cd /srv/www/deploy && docker compose up api -d --force-recreate"
                    '''
                }
            }
        }
    }
}
