#!/usr/bin/env groovy

// Searches previous builds to find first non aborted one
def getLastNonAbortedBuild(build) {
  if (build == null) {
    return null;
  }

  if(build.result.toString().equals("ABORTED")) {
    return getLastNonAbortedBuild(build.getPreviousBuild());
  } else {
    return build;
  }
}

// Send out a slack message if branch got broken or has recovered
def notifySlackUponStateChange(build) {
  def cur = build.getResult()
  def prev = getLastNonAbortedBuild(build.getPreviousBuild())?.getResult()
  if (cur != prev) {
    if (cur == 'SUCCESS') {
      slackSend(
        channel: '#mayastor',
        color: 'normal',
        message: "Branch ${env.BRANCH_NAME} has been fixed :beers: (<${env.BUILD_URL}|Open>)"
      )
    } else if (prev == 'SUCCESS') {
      slackSend(
        channel: '#mayastor',
        color: 'danger',
        message: "Branch ${env.BRANCH_NAME} is broken :face_with_raised_eyebrow: (<${env.BUILD_URL}|Open>)"
      )
    }
  }
}

def mainBranches() {
    return BRANCH_NAME == "develop" || BRANCH_NAME.startsWith("release/");
}

// TODO: Use multiple choices
run_linter = true
rust_test = true
grpc_test = true
pytest_test = true
// WA https://issues.jenkins.io/browse/JENKINS-41929
// on the first run of new parameters, they are set to null.
run_tests = params.run_tests == null ? true : params.run_tests
build_images = params.build_images == null ? false : params.build_images

if (currentBuild.getBuildCauses('jenkins.branch.BranchIndexingCause') && mainBranches()) {
  print "INFO: Branch Indexing, skip tests and push the new images."
  run_tests = false
  build_images = true
}

// Only schedule regular builds on main branches, so we don't need to guard against it
String cron_schedule = mainBranches() ? "0 2 * * *" : ""

pipeline {
  agent none
  options {
    timeout(time: 5, unit: 'HOURS')
    skipDefaultCheckout()
  }
  parameters {
    booleanParam(defaultValue: false, name: 'build_images')
    booleanParam(defaultValue: true, name: 'run_tests')
  }
  triggers {
    cron(cron_schedule)
  }

  stages {
    stage('init') {
      agent { label 'nixos-mayastor' }
      steps {
        cleanWs()
        checkout([
          $class: 'GitSCM',
          branches: scm.branches,
          extensions: scm.extensions.findAll {
            !(it instanceof jenkins.plugins.git.GitSCMSourceDefaults)
          } + [[
            $class: 'CloneOption',
            noTags: false,
            reference: '', shallow: false
          ], [
            $class: 'SubmoduleOption',
            disableSubmodules: false,
            parentCredentials: true,
            recursiveSubmodules: true,
            reference: '',
            trackingSubmodules: false
          ]],
          userRemoteConfigs: scm.userRemoteConfigs
        ])
        stash name: 'source', useDefaultExcludes: false
        step([
          $class: 'GitHubSetCommitStatusBuilder',
          contextSource: [
            $class: 'ManuallyEnteredCommitContextSource',
            context: 'continuous-integration/jenkins/branch'
          ],
          statusMessage: [ content: 'Pipeline started' ]
        ])
      }
    }
    stage('linter') {
      agent { label 'nixos-mayastor' }
      when {
        beforeAgent true
        not {
          anyOf {
            branch 'master'
            branch 'release/*'
            expression { run_linter == false }
          }
        }
        expression { run_tests == true }
      }
      steps {
        cleanWs()
        unstash 'source'
        sh 'nix-shell --run "./scripts/rust-style.sh" ci.nix'
        sh 'nix-shell --run "./scripts/rust-linter.sh" ci.nix'
        sh 'nix-shell --run "./scripts/js-check.sh" ci.nix'
      }
    }
    stage('test') {
      when {
        beforeAgent true
        not {
          anyOf {
            branch 'master'
          }
        }
        expression { run_tests == true }
      }
      parallel {
        stage('rust unit tests') {
          when {
            beforeAgent true
            expression { rust_test == true }
          }
          agent { label 'nixos-mayastor' }
          environment {
            START_DATE = new Date().format("yyyy-MM-dd HH:mm:ss", TimeZone.getTimeZone('UTC'))
          }
          steps {
            cleanWs()
            unstash 'source'
            sh 'printenv'
            sh 'nix-shell --run "./scripts/cargo-test.sh" ci.nix'
          }
          post {
            always {
              // in case of abnormal termination of any nvmf test
              sh 'sudo nvme disconnect-all'
              sh 'sudo ./scripts/check-coredumps.sh --since "${START_DATE}"'
            }
          }
        }
        stage('image build test') {
          when {
            branch 'staging'
          }
          agent { label 'nixos-mayastor' }
          steps {
            cleanWs()
            unstash 'source'
            sh 'printenv'
            sh './scripts/release.sh --skip-publish --debug'
          }
        }
        stage('grpc tests') {
          when {
            beforeAgent true
            expression { grpc_test == true }
          }
          agent { label 'nixos-mayastor' }
          environment {
            START_DATE = new Date().format("yyyy-MM-dd HH:mm:ss", TimeZone.getTimeZone('UTC'))
          }
          steps {
            cleanWs()
            unstash 'source'
            sh 'printenv'
            sh 'nix-shell --run "./scripts/grpc-test.sh" ci.nix'
          }
          post {
            always {
              junit '*-xunit-report.xml'
              sh 'sudo ./scripts/check-coredumps.sh --since "${START_DATE}"'
            }
          }
        }
        stage('pytest tests') {
          when {
            beforeAgent true
            expression { pytest_test == true }
          }
          agent { label 'virtual-nixos-mayastor' }
          stages {
            stage('checkout') {
              steps {
                cleanWs()
                checkout([
                  $class: 'GitSCM',
                  branches: scm.branches,
                  extensions: scm.extensions.findAll {
                    !(it instanceof jenkins.plugins.git.GitSCMSourceDefaults)
                  } + [[
                    $class: 'CloneOption',
                    noTags: false,
                    reference: '',
                    shallow: false
                  ], [
                    $class: 'SubmoduleOption',
                    disableSubmodules: false,
                    parentCredentials: true,
                    recursiveSubmodules: true,
                    reference: '',
                    trackingSubmodules: false
                  ]],
                  userRemoteConfigs: scm.userRemoteConfigs
                ])
              }
            }
            stage('build') {
              steps {
                sh 'printenv'
                sh 'nix-shell --run "cargo build --bins" ci.nix'
              }
            }
            stage('python setup') {
              steps {
                sh 'nix-shell --run "./test/python/setup.sh" ci.nix'
              }
            }
            stage('run tests') {
              steps {
                sh 'printenv'
                // Cleanup any existing containers.
                // They could be lingering if there were previous test failures.
                sh 'docker system prune -f'
                sh 'nix-shell --run "./scripts/pytest-tests.sh" ci.nix'
                sh 'docker system prune -f'
              }
            }
          }
        }
      }// parallel stages block
    }// end of test stage

    stage('build and push images') {
      agent { label 'nixos-mayastor' }
      when {
        beforeAgent true
        anyOf {
          expression { build_images == true }
          anyOf {
            branch 'master'
            branch 'release/*'
            branch 'develop'
          }
        }
      }
      steps {
        sh 'printenv'
        // Clean the workspace and unstash the source to ensure we build and push the correct images.
        cleanWs()
        unstash 'source'

        withCredentials([usernamePassword(credentialsId: 'OPENEBS_DOCKERHUB', usernameVariable: 'USERNAME', passwordVariable: 'PASSWORD')]) {
          sh 'echo $PASSWORD | docker login -u $USERNAME --password-stdin'
        }
        sh './scripts/release.sh'
      }
      post {
        always {
          sh 'docker logout'
          sh 'docker image prune --all --force'
        }
      }
    }
  }

  // The main motivation for post block is that if all stages were skipped
  // (which happens when running cron job and branch != mainBranches()) then we don't
  // want to set commit status in github (jenkins will implicitly set it to
  // success).
  post {
    always {
      node(null) {
        script {
          // If no tests were run then we should neither be updating commit
          // status in github nor send any slack messages
          if (currentBuild.result != null) {
            step([
              $class: 'GitHubCommitStatusSetter',
              errorHandlers: [[$class: "ChangingBuildStatusErrorHandler", result: "UNSTABLE"]],
              contextSource: [
                $class: 'ManuallyEnteredCommitContextSource',
                context: 'continuous-integration/jenkins/branch'
              ],
              statusResultSource: [
                $class: 'ConditionalStatusResultSource',
                results: [
                  [$class: 'AnyBuildResult', message: 'Pipeline result', state: currentBuild.getResult()]
                ]
              ]
            ])
            if (mainBranches()) {
              notifySlackUponStateChange(currentBuild)
            }
          }
        }
      }
    }
  }
}
