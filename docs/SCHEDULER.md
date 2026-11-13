# Reliable scheduling for the Jazz FM collector

GitHub's own `schedule:` cron is best-effort: runs are often delayed or skipped, and the Jazz FM
page only shows the latest 5 plays, so a skipped run can mean missing plays. A free hosted
scheduler (nothing to run at home) can start the workflow on time instead. The workflow already
accepts `workflow_dispatch`, so no code changes are needed. GitHub's cron stays as a backstop;
overlapping runs are harmless because plays are de-duplicated and runs are queued, not cancelled.

## One-time setup

1. **Token.** GitHub > Settings > Developer settings > Fine-grained tokens > Generate new token.
   - Repository access: only `pink-bourbon-estate/Radio_Archives`
   - Permissions: Repository > **Actions: Read and write** (nothing else)
   - Expiry: the longest offered. Put a reminder in your calendar to renew it.
2. **Account.** Sign up at https://cron-job.org (free).
3. **Cron job.** Create one with:
   - URL: `https://api.github.com/repos/pink-bourbon-estate/Radio_Archives/actions/workflows/jazz-fm.yml/dispatches`
   - Schedule: every 15 minutes (pick odd minutes such as 4, 19, 34, 49 to avoid the busy :00/:15)
   - Advanced > Request method: `POST`
   - Advanced > Headers:
     - `Authorization: Bearer YOUR_TOKEN`
     - `Accept: application/vnd.github+json`
     - `X-GitHub-Api-Version: 2022-11-28`
     - `Content-Type: application/json`
   - Advanced > Request body: `{"ref":"main"}`
   - Turn on failure notifications so you hear if the token expires.
4. Use "Test run". A success is HTTP **204**, and a new "Jazz FM collector" run appears in the repo's
   Actions tab within seconds.

The collector itself is unchanged: it still reads only the public playlist page, checks
robots.txt first, and identifies itself as RadioArchivesBot.
