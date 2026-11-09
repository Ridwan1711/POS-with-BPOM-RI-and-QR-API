-- Open Food Facts: barcode product database (ODbL). Disabled until OFF_USER_AGENT is configured.
-- Rate limit matches OFF's published read limit (15 requests per minute per IP).

INSERT INTO external_product_sources (code, name, description, enabled, priority, requires_credentials, rate_limit_per_minute)
VALUES (
    'open_food_facts',
    'Open Food Facts',
    'Crowdsourced product database, ODbL licence. Attribution required. Requires a contact User-Agent (OFF_USER_AGENT). See docs/integrations/open-food-facts.md.',
    FALSE,
    5,
    FALSE,
    15
)
ON CONFLICT (code) DO NOTHING;
