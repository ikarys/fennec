# Insight

Read model usage, estimated costs, project budgets, and verified indicators from the **Insight** page and the project page.

## Usage

The **Insight** page shows **Model usage over the period**. Choose **7 days**, **30 days**, **90 days**, or **All**, and filter by **All projects** or a project.

The usage view shows **Input per day** and a legend for **cache**, **non-cached**, and **lost cache**. The cache legend shows the part of the input that was reused, the part that was not, and cache that was lost. **What consumes the input** and **By project** break the usage down. The table shows **Lost cache**, **Cost**, and **Time**. If no model calls were recorded, it shows **No calls over the period.**

The usage ledger keeps calls, tokens, and speeds beyond individual sessions. In a session, the **Session tokens** panel shows **Calls**, **Estimated cost**, **Last call**, and **Composition**. Speeds are **Estimated from the received text, measured at the end of the message**.

## Costs and pricing

Model prices are set in **Settings** > **Models** under **Price ($ per million tokens, for the Insight estimate)**. Prices are per million tokens for input, cached input, and output.

**Estimated cost** uses those prices. The cost is an estimate; the usage figures are recorded facts.

The **Billed** checkbox controls whether a model counts toward project budgets. It is off by default for a local model: its price is shown as an estimate but does not block a turn. A paid provider's model is billed by default. You can override either default with **Billed**.

## Monthly budgets

Set a monthly cost budget from the project page tile **Set the monthly budget**. Enter a value in **Monthly budget in $**, then click **Save the budget** or **Remove the budget**. **Cancel** closes the dialog.

The month's spend is the usage ledger priced at each model's price. At 80 % of the budget, fennec raises an alert. Once the budget is exceeded, fennec refuses to start a new turn on that project until you raise or remove the budget. Your typed message stays in the composer.

Only billed models count toward the budget. Every model's price remains displayed.

## Indicators

The **Indicators** tab on the project page measures the project from recorded facts only. Its hint is **Measured from what fennec recorded (integrations, dialogues, git, the usage ledger), never from what a model says.**

It reports, among other facts:

- AI commits outside fennec
- sensitive integrations without a comprehension dialogue
- comprehension dialogues held
- module rule breaks, with accepted violations and active waivers
- security scanner findings accepted
- the month's cost per integration
- the state of `docs/map.md` and ADRs
- integrations tied to an approved plan
- blockers accepted per integration

## Dashboard

The project dashboard gives verified tiles for **Modules**, **Dependencies**, **Module rules**, and **Since docs/map.md**. The budget tile lets you open **Set the monthly budget**. Use [understanding](understanding.md) to read the map, rules, and risk zones behind those tiles.
