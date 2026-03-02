@CompileStatic
pipeline {
    agent any

    stages {
        stage('Build') {
            steps {
                echo 'Building...'
                sh 'docker save novabyte-api:latest -o nb-api_docker-image'
                // sh 'xz -T0 -9 > ~/temp/nb-blog_api'
                zip zipFile: 'nb-blog_api', file: 'nb-api_docker-image'
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
