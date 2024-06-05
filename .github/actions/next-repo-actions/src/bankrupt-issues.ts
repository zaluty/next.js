import { context, getOctokit } from '@actions/github'
import { info, setFailed } from '@actions/core'

async function main() {
  if (!process.env.GITHUB_TOKEN) throw new TypeError('GITHUB_TOKEN not set')

  const octokit = getOctokit(process.env.GITHUB_TOKEN)
  const { owner, repo } = context.repo
  const commentBody = `

We are in the process of closing issues dating back to 2020 to improve our focus on the most relevant and actionable problems.

**_Why are we doing this?_**

Stale issues often lack recent updates and clear reproductions, making them difficult to address effectively. Our objective is to prioritize the most upvoted and actionable issues that have up-to-date reproductions, enabling us to resolve bugs more efficiently.

**_Why 2020 issues?_**

Issues from 2020 are likely to be outdated and less relevant to the current state of the codebase. By closing these older stale issues, we can better focus our efforts on more recent and relevant problems, ensuring a more effective and streamlined workflow.

If your issue is still relevant, please reopen it using our [bug report template](https://github.com/vercel/next.js/issues/new?assignees=&labels=bug&projects=&template=1.bug_report.yml). Be sure to include any important context from the original issue in your new report.

Thank you for your understanding and contributions.

Best regards,
The Next.js Team
  `

  try {
    await octokit.rest.issues.createComment({
      owner,
      repo,
      issue_number: 66573,
      body: commentBody,
    })

    // await octokit.rest.issues.addAssignees({
    //   owner,
    //   repo,
    //   issue_number: 66573,
    //   assignees: ['samcx'],
    // })

    info(`Commented on issue #66573`)
  } catch (error) {
    setFailed(error)
  }
}

main()
