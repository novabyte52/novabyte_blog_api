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
                        ssh -i "$PK" ${DROPLET_USER}@${DROPLET_HOST} "cd /srv/www/deploy && docker compose restart api"
                    '''
                }
            }
        }
    }
}
