# ADR 0004: Web-search provider disposition

- Status: Approved for current release
- Date: 2026-08-17
- Owners: Product, privacy, and platform

## Decision

Ark keeps Brave Search as the single provisional web-search backend. Ark does not implement Zenserp or a provider registry until a second backend or user-selectable endpoint is approved. The existing tool capability, OS secret store, audit trail, bounded citation contract, and untrusted-context boundary remain provider-neutral.

## Evidence reviewed

Brave's public Search API pricing lists $5 per 1,000 requests and a $5 monthly credit; an account and payment details are required. Brave's API privacy notice says query records may be retained for up to 90 days, with zero-data-retention offered separately. Zenserp's public plan lists 50 free searches per month and paid service from $49.99 for 25,000 searches. Its linked general Idera privacy notice does not state a Zenserp-specific query-retention ceiling.

Kagi was evaluated as a credible independent-index alternative. Its current API documentation lists $12 per 1,000 searches, requires an account and prepaid API balance, and offers no documented recurring free API allowance. Kagi's privacy notice says sampled load-balancer and virtual-machine request logs are retained for seven days and that customer data is not cached; sampled server-error records can remain in Sentry for 90 days. This is a stronger documented privacy posture than the other candidates, but its higher per-query cost and lack of a free allowance make it unsuitable as Ark's default today. A user-supplied endpoint was also rejected for this release because Ark could not truthfully provide uniform privacy, citation-quality, or failure guarantees without an approved adapter contract.

Sources, verified 2026-08-17:

- https://brave.com/search/api/
- https://api-dashboard.search.brave.com/privacy-policy
- https://zenserp.com/pricing-plans/
- https://www.ideracorp.com/en/legal/privacypolicy
- https://help.kagi.com/kagi/api/api-portal.html
- https://kagi.com/privacy

## Consequences

- Settings must disclose destination, account/cost class, and the 90-day maximum before a grant is issued.
- Pricing and terms must be rechecked for every release because they are mutable.
- Live-network tests remain outside CI; adapters use bounded deterministic fixtures for authentication, quota, malformed response, oversized response, timeout, and redirect behavior.
- Search results remain untrusted context and cannot authorize tool execution.
- A future second backend must implement a narrow adapter returning Ark's existing `SearchCitation` shape; vendor-name switching must not enter generation or tool policy code.
