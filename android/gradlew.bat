@rem Gradle wrapper script for Windows

@if "%DEBUG%"=="" @echo off

set DIRNAME=%~dp0
set APP_HOME=%DIRNAME%

set CLASSPATH=%APP_HOME%\gradle\wrapper\gradle-wrapper.jar

@rem Find java.exe
set JAVA_EXE=java.exe
if defined JAVA_HOME goto findJavaFromJavaHome
goto execute

:findJavaFromJavaHome
set JAVA_HOME=%JAVA_HOME:"=%
set JAVA_EXE=%JAVA_HOME%/bin/java.exe

:execute
"%JAVA_EXE%" -classpath "%CLASSPATH%" org.gradle.wrapper.GradleWrapperMain %*

:end
