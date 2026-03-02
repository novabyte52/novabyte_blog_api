pipeline {
    agent any

    stages {
        stage('Build') {
            steps {
                echo 'Building...'
                sh 'docker save -o nb-api_docker-image.tar novabyte-api:latest'
                sh 'xz -T0 -9 nb-api_docker-image.tar > nb-blog_api'
                // zip zipFile: 'nb-blog_api.zip', dir: '.', file: 'nb-api_docker-image.tar'
            }
        }
        // stage('Test') {
        //     steps {
        //         echo 'Testing...'
        //     }
        // }
        // stage('Deploy') {
        //     environment {
        //         USER=credentials('')
        //         PASSWORD=credentials('')
        //     }
        //     steps {
        //         echo 'Deploying...'

        //     }
        // }
    }
}
