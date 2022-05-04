# Voter Stake Registry Changelog

## v0.2.4 - Unreleased

### Program
- Upgrade Anchor to v0.24.2

## v0.2.3 - 2022-4-29

### Program
- Use spl-governance 2.2.1 as dependency, instead of a specific commit.

### Typescript Client
- Upgrade the Anchor dependency to v0.24.2

## v0.2.2 - skipped

## v0.2.1 - 2022-4-3

### Program
- Increase the maximum number of lockup periods to 200 * 365 to allow for 200-year cliff and
  constant lockups.
- Add a function to compute the guaranteed locked vote power bonus. This is unused by the
  program itself, but helpful for programs that want to provide benefits based on a user's
  lockup amount and time.

### Other
- Add cli tool to decode voter accounts.
- Update dependencies.


## v0.2.0 - 2022-2-14

- First release.
- Available on devnet at 4Q6WW2ouZ6V3iaNm56MTd5n2tnTm4C5fiH8miFHnAFHo
- In use by the Mango DAO on mainnet at the same address.
