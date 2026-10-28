# Security policy

GetLocked runs on people's PCs and closes processes, so security bugs matter. Examples include:

- a web page being able to talk to the local WebSocket bridge
- a way to make GetLocked close processes it shouldn't (other users', system processes)
- anything that lets a non-extension process change or end a session

## Reporting

Please **don't open a public issue** for security problems. Use GitHub's [private vulnerability reporting](../../security/advisories/new) on this repository instead. Include steps to reproduce and the GetLocked version.

You'll get a reply within a week. Fixes are released as soon as they're ready, and reporters are credited unless they'd rather not be.

## Not in scope

Ways to get out of your *own* Hard-mode session (for example, killing both processes and deleting the session file). Hard mode is documented as "reasonable effort", not a security boundary. Ideas to make it sturdier without admin rights are welcome as normal issues.
