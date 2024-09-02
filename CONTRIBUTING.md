# Edgee Contributor Guidelines

Welcome! This project is created by the team at [Edgee](https://www.edgee.cloud). 
We're glad you're interested in contributing! We welcome contributions from people of all backgrounds 
who are interested in making great software with us.

At Edgee, we aspire to empower everyone to create interactive experiences. To do this, 
we're exploring and pushing the boundaries of new technologies, and sharing our learnings with the open source community.

If you have ideas for collaboration, email us at opensource@edgee.cloud or join our [Slack](https://www.edgee.cloud/slack)!

We're also hiring full-time engineers to work with us everywhere! Check out our current job postings [here](https://github.com/edgee-cloud/careers/issues).

## Issues

### Feature Requests

If you have ideas or how to improve our projects, you can suggest features by opening a GitHub issue. 
Make sure to include details about the feature or change, and describe any uses cases it would enable.

Feature requests will be tagged as `enhancement` and their status will be updated in the comments of the issue.

### Bugs

When reporting a bug or unexpected behavior in a project, make sure your issue describes steps 
to reproduce the behavior, including the platform you were using, what steps you took, and any error messages.

Reproducible bugs will be tagged as `bug` and their status will be updated in the comments of the issue.

### Wontfix

Issues will be closed and tagged as `wontfix` if we decide that we do not wish to implement it, 
usually due to being misaligned with the project vision or out of scope. We will comment on the issue with more detailed reasoning.

## Contribution Workflow

### Open Issues

If you're ready to contribute, start by looking at our open issues tagged as [`help wanted`](../../issues?q=is%3Aopen+is%3Aissue+label%3A"help+wanted") or [`good first issue`](../../issues?q=is%3Aopen+is%3Aissue+label%3A"good+first+issue").

You can comment on the issue to let others know you're interested in working on it or to ask questions.

### Making Changes

1. Fork the repository.

2. Review the [Development Workflow](#development-workflow) section to understand how to run the project locally.

3. Create a new feature [branch](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository).

4. Make your changes on your branch. Ensure that there are no build errors by running the project with your changes locally.

5. [Submit the branch as a Pull Request](https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/creating-a-pull-request-from-a-fork) pointing to the `main` branch of the Edgee repository. A maintainer should comment and/or review your Pull Request within a few days. Although depending on the circumstances, it may take longer.

### Development Workflow

#### Setup and run Edgee

```bash
cargo run
```

#### Test

```bash
cargo test
```

This command will be triggered to each PR as a requirement for merging it.


## Licensing

Unless otherwise specified, all Edgee open source projects shall comply with the Apache 2.0 licence. Please see the [LICENSE](LICENSE) file for more information.

## Contributor Terms

Thank you for your interest in Edgee’ open source project. By providing a contribution (new or modified code, 
other input, feedback or suggestions etc.) you agree to these Contributor Terms.

You confirm that each of your contributions has been created by you and that you are the copyright owner. 
You also confirm that you have the right to provide the contribution to us and that you do it under the 
Apache 2.0 licence.

If you want to contribute something that is not your original creation, you may submit it to Edgee separately 
from any contribution, including details of its source and of any license or other restriction 
(such as related patents, trademarks,  agreements etc.)

Please also note that our projects are released with a [Contributor Code of Conduct](CODE_OF_CONDUCT.md) to 
ensure that they are welcoming places for everyone to contribute. By participating in any Edgee open source project, 
you agree to keep to the Contributor Code of Conduct.
