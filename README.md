# todiff - human-readable diff for todo.txt files

`todiff` provides a human-readable diff tool for [todo.txt](http://todotxt.org)
files.

It can be used with `todiff <BEFORE> <AFTER>`.

Note that the output is not designed to be parsed by script, thus can change
arbitrarily without it being considered a breaking change. Changing the way the
executable is called, on the other hand, is considered a breaking change, so
that eg. crons can be used to periodically diff.

## Example usage

```
$ # `git diff` is not really readable…

$ git diff
diff --git a/todo.txt b/todo.txt
index 547d201..66caa7e 100644
--- a/todo.txt
+++ b/todo.txt
@@ -1,2 +1,3 @@
-2018-03-20 Take over the world t:2022-02-02 due:2033-03-03
-2018-03-20 Call mom due:2018-03-25
+2018-03-20 Take over the world t:2022-02-05 due:2033-03-06
+(A) 2018-03-20 Call mom due:2018-03-25
+2018-03-20 Be happy t:2099-09-09

$ # But with todiff it's quite better!

$ git difftool -x todiff -y
New tasks:
 → 2018-03-20 Be happy t:2099-09-09

Changed tasks:
 → 2018-03-20 Take over the world due:2033-03-03 t:2022-02-02
    → Postponed (strict) by 3 days

 → 2018-03-20 Call mom due:2018-03-25
    → Added priority (A)

```

You can then for example have a daily cron similar to this:
```bash
#!/bin/sh

cd $MY_TODO_TXT_GIT_REPO
git add todo.txt
git difftool --cached -x todiff -y
git commit -m "$(date -I)"
```

This will automatically send you an email (provided your cron daemon is
correctly configured) with all the task changes you did during the day, and
commit the todo.txt file for backup as well as future use.
