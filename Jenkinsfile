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
                sh 'echo "loading env file..."'
                withCredentials([file(credentialsId: 'nb-blog-env-file', variable: 'ENV_FILE')]) { }
                sh 'echo "loaded env file...."'
            }
        }

        stage('Build') {
            steps {
                echo 'Building...'
                sh 'docker build -t novabyte-api:latest .'
                sh 'docker save -o nb-api_docker-image.tar novabyte-api:latest'
                sh 'xz -T0 -9 nb-api_docker-image.tar > nb-blog_api'
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
                        scp -i $PK nb-blog_api ${DROPLET_USER}@${DROPLET_HOST}:${DEPLOY_PATH}/
                        ssh -i $PK ${DROPLET_USER}@${DROPLET_HOST} "cd ${DEPLOY_PATH} && xz -d nb-blog_api && docker load -i nb-api_docker-image.tar"
                    '''
                }
            }
        }
    }
}
