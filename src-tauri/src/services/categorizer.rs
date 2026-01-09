use regex::Regex;

use crate::models::{CategoryRule, Transaction};

pub struct Categorizer {
    rules: Vec<CompiledRule>,
}

struct CompiledRule {
    category_id: String,
    pattern: String,
    regex: Option<Regex>,
    field: String,
    priority: i32,
}

impl Categorizer {
    pub fn new(rules: Vec<CategoryRule>) -> Self {
        let compiled_rules: Vec<CompiledRule> = rules
            .into_iter()
            .map(|rule| {
                let regex = if rule.is_regex {
                    Regex::new(&rule.pattern).ok()
                } else {
                    None
                };

                CompiledRule {
                    category_id: rule.category_id,
                    pattern: rule.pattern.to_lowercase(),
                    regex,
                    field: rule.field,
                    priority: rule.priority,
                }
            })
            .collect();

        Self { rules: compiled_rules }
    }

    pub fn categorize(&self, transaction: &Transaction) -> Option<String> {
        let mut best_match: Option<(&CompiledRule, i32)> = None;

        for rule in &self.rules {
            let field_value = match rule.field.as_str() {
                "description" => &transaction.description,
                "payee" => transaction.payee.as_ref().unwrap_or(&transaction.description),
                _ => &transaction.description,
            };

            let field_lower = field_value.to_lowercase();

            let matches = if let Some(ref regex) = rule.regex {
                regex.is_match(&field_lower)
            } else {
                field_lower.contains(&rule.pattern)
            };

            if matches {
                if best_match.is_none() || rule.priority > best_match.unwrap().1 {
                    best_match = Some((rule, rule.priority));
                }
            }
        }

        best_match.map(|(rule, _)| rule.category_id.clone())
    }

    pub fn categorize_batch(&self, transactions: &mut [Transaction]) {
        for tx in transactions.iter_mut() {
            if tx.category_id.is_none() {
                tx.category_id = self.categorize(tx);
            }
        }
    }
}

pub fn get_default_rules() -> Vec<CategoryRule> {
    let mut rules = Vec::new();

    // ============================================================
    // HIGH PRIORITY RULES (priority 10) - Specific patterns that
    // should override generic matches
    // ============================================================

    // Credit Card Payments - HIGH PRIORITY to catch before other patterns
    for pattern in [
        "capital one", "capitalone", "chase card", "chase credit", "discover card",
        "american express", "amex", "citi card", "citicard", "barclays card",
        "bank of america card", "wells fargo card", "credit card payment",
        "card payment", "card services", "synchrony", "credit one",
    ] {
        rules.push(CategoryRule::with_priority("cat_expense_credit_card".into(), pattern.into(), "description".into(), 10));
    }

    // Fuel at stores - HIGH PRIORITY to override grocery store matches
    // These patterns specifically identify fuel purchases at multi-purpose stores
    for pattern in [
        "kroger fuel", "walmart fuel", "costco gas", "costco fuel", "sam's fuel",
        "sam's gas", "safeway fuel", "albertsons fuel", "heb fuel", "h-e-b fuel",
        "meijer fuel", "giant fuel", "stop & shop fuel", "bj's gas", "bj's fuel",
        "murphy usa", "murphy oil", "kum & go", "kwik trip fuel",
        "sheetz gas", "raceway", "thorntons", "getgo", "giant eagle fuel",
        // WaWa and Sheetz are primarily gas stations/convenience stores
        "wawa", "sheetz",
    ] {
        rules.push(CategoryRule::with_priority("cat_expense_gas".into(), pattern.into(), "description".into(), 10));
    }

    // Loan Payments - HIGH PRIORITY
    for pattern in [
        "student loan", "car loan", "auto loan", "personal loan", "loan payment",
        "sallie mae", "navient", "nelnet", "great lakes", "fedloan", "sofi loan",
    ] {
        rules.push(CategoryRule::with_priority("cat_expense_loan".into(), pattern.into(), "description".into(), 10));
    }

    // ============================================================
    // NORMAL PRIORITY RULES (priority 0) - Generic patterns
    // ============================================================

    // Income - Salary
    for pattern in ["payroll", "direct deposit", "salary", "wages", "paychex", "adp", "gusto payroll"] {
        rules.push(CategoryRule::new("cat_income_salary".into(), pattern.into(), "description".into()));
    }

    // Income - Investment
    for pattern in ["dividend", "interest payment", "capital gain", "investment return", "stock sale"] {
        rules.push(CategoryRule::new("cat_income_investment".into(), pattern.into(), "description".into()));
    }

    // Housing - Mortgage
    for pattern in ["mortgage", "home loan", "quicken loans", "rocket mortgage", "wells fargo home", "chase mortgage", "bank of america mortgage"] {
        rules.push(CategoryRule::new("cat_expense_mortgage".into(), pattern.into(), "description".into()));
    }

    // Housing - Rent
    for pattern in ["rent payment", "apartment rent", "landlord", "property management", "zillow rent"] {
        rules.push(CategoryRule::new("cat_expense_housing".into(), pattern.into(), "description".into()));
    }

    // Insurance
    for pattern in ["insurance", "geico", "state farm", "allstate", "progressive", "liberty mutual", "farmers insurance", "nationwide", "usaa", "metlife", "aetna", "cigna", "united health", "blue cross", "blue shield", "kaiser", "humana", "anthem"] {
        rules.push(CategoryRule::new("cat_expense_insurance".into(), pattern.into(), "description".into()));
    }

    // Utilities (note: "gas bill" and "natural gas" are utilities, not fuel)
    for pattern in ["utility", "utilities", "electric", "electricity", "power company", "pge", "pg&e", "con edison", "duke energy", "water bill", "water utility", "sewer", "gas bill", "natural gas", "internet", "broadband", "comcast", "xfinity", "spectrum", "cox", "centurylink", "frontier", "fios"] {
        rules.push(CategoryRule::new("cat_expense_utilities".into(), pattern.into(), "description".into()));
    }

    // Cell Phone / Mobile Plans
    for pattern in ["t-mobile", "verizon wireless", "verizon mobile", "at&t wireless", "at&t mobility", "sprint", "metro pcs", "metropcs", "mint mobile", "cricket wireless", "boost mobile", "us cellular", "google fi", "visible wireless", "straight talk", "tracfone", "consumer cellular", "ting mobile", "republic wireless"] {
        rules.push(CategoryRule::new("cat_expense_phone".into(), pattern.into(), "description".into()));
    }

    // Professional & Technology Services (AI, hosting, development tools)
    for pattern in ["anthropic", "claude", "openai", "chatgpt", "github copilot", "aws", "amazon web services", "google cloud", "azure", "digitalocean", "linode", "vultr", "heroku", "netlify", "vercel", "cloudflare", "namecheap", "godaddy", "hover", "squarespace", "wix", "webflow", "figma", "linear", "jira", "confluence", "asana", "monday.com", "basecamp", "trello"] {
        rules.push(CategoryRule::new("cat_expense_professional".into(), pattern.into(), "description".into()));
    }

    // Groceries (removed stores that also sell fuel - those need HIGH PRIORITY fuel rules)
    for pattern in ["grocery", "supermarket", "whole foods", "trader joe", "safeway", "kroger", "publix", "albertsons", "aldi", "food lion", "harris teeter", "wegmans", "h-e-b", "meijer", "stop & shop", "giant", "food mart", "fresh market", "sprouts", "instacart", "shipt", "walmart grocery", "amazon fresh", "costco wholesale", "sam's club"] {
        rules.push(CategoryRule::new("cat_expense_groceries".into(), pattern.into(), "description".into()));
    }

    // Dining Out - Restaurants
    for pattern in ["restaurant", "ristorante", "bistro", "diner", "grill", "kitchen", "eatery", "pizzeria", "sushi", "thai", "chinese", "mexican", "italian", "indian", "taqueria", "burrito", "tacos"] {
        rules.push(CategoryRule::new("cat_expense_dining".into(), pattern.into(), "description".into()));
    }

    // Dining - Fast Food
    for pattern in ["mcdonald", "burger king", "wendy's", "taco bell", "chick-fil-a", "popeyes", "kfc", "subway", "jimmy john", "jersey mike", "firehouse subs", "five guys", "shake shack", "in-n-out", "whataburger", "sonic", "jack in the box", "carl's jr", "hardee's", "arby's", "dairy queen", "domino", "pizza hut", "papa john", "little caesars", "panda express", "chipotle", "qdoba", "wingstop", "buffalo wild wings", "panera", "noodles & co", "jason's deli", "zaxby"] {
        rules.push(CategoryRule::new("cat_expense_dining".into(), pattern.into(), "description".into()));
    }

    // Dining - Delivery
    for pattern in ["doordash", "uber eats", "grubhub", "postmates", "seamless", "caviar", "gopuff", "delivery.com", "slice pizza"] {
        rules.push(CategoryRule::new("cat_expense_dining".into(), pattern.into(), "description".into()));
    }

    // Coffee & Cafes (removed generic "cafe" to avoid matching non-coffee places)
    for pattern in ["starbucks", "dunkin", "peet's coffee", "coffee bean", "blue bottle", "philz", "dutch bros", "caribou coffee", "tim hortons", "cafe nero", "costa coffee", "coffee shop", "espresso"] {
        rules.push(CategoryRule::new("cat_expense_coffee".into(), pattern.into(), "description".into()));
    }

    // Gas & Fuel - gas station brands and generic fuel terms
    for pattern in ["shell", "chevron", "exxon", "mobil", "bp", "arco", "sunoco", "circle k", "speedway", "7-eleven", "quiktrip", "racetrac", "pilot", "flying j", "love's travel", "marathon", "valero", "gas station", "fuel center", "petrol", "gasoline"] {
        rules.push(CategoryRule::new("cat_expense_gas".into(), pattern.into(), "description".into()));
    }

    // Rideshare
    for pattern in ["uber trip", "lyft", "via ride"] {
        rules.push(CategoryRule::new("cat_expense_rideshare".into(), pattern.into(), "description".into()));
    }

    // Transportation - Other
    for pattern in ["parking", "toll", "transit", "metro", "bus", "train", "railway", "amtrak", "greyhound", "enterprise rent", "hertz", "avis", "budget car", "national car", "zipcar", "turo"] {
        rules.push(CategoryRule::new("cat_expense_transportation".into(), pattern.into(), "description".into()));
    }

    // Streaming Services (Video)
    for pattern in [
        "netflix", "hulu", "disney+", "disney plus", "disneyplus", "hbo max", "max streaming",
        "amazon prime video", "prime video", "peacock", "paramount+", "paramount plus",
        "apple tv", "youtube premium", "youtube tv", "google yt premium", "yt premium",
        "crunchyroll", "funimation", "discovery+", "showtime", "starz", "fubo", "sling tv",
        "philo", "tubi", "amazon prime", "prime membership",
    ] {
        rules.push(CategoryRule::new("cat_expense_streaming".into(), pattern.into(), "description".into()));
    }

    // Music Services
    for pattern in ["spotify", "apple music", "tidal", "amazon music", "pandora", "deezer", "youtube music", "soundcloud go", "audible"] {
        rules.push(CategoryRule::new("cat_expense_music".into(), pattern.into(), "description".into()));
    }

    // Gaming Subscriptions
    for pattern in ["xbox game pass", "playstation plus", "ps plus", "nintendo online", "ea play", "ubisoft+", "xbox live", "steam", "epic games", "twitch sub", "discord nitro"] {
        rules.push(CategoryRule::new("cat_expense_gaming".into(), pattern.into(), "description".into()));
    }

    // News & Magazines
    for pattern in ["new york times", "nytimes", "washington post", "wall street journal", "wsj", "the atlantic", "economist", "wired", "bloomberg", "financial times", "medium", "substack", "apple news", "kindle unlimited", "scribd", "espn+", "the athletic", "reuters", "associated press", "politico"] {
        rules.push(CategoryRule::new("cat_expense_news".into(), pattern.into(), "description".into()));
    }

    // Apps & Software
    for pattern in ["app store", "google play", "microsoft 365", "office 365", "adobe", "creative cloud", "dropbox", "icloud", "google one", "evernote", "notion", "slack", "zoom", "github", "gitlab", "jetbrains", "1password", "lastpass", "dashlane", "nordvpn", "expressvpn", "surfshark", "cloudflare", "canva", "grammarly", "chatgpt", "openai"] {
        rules.push(CategoryRule::new("cat_expense_apps".into(), pattern.into(), "description".into()));
    }

    // Healthcare
    for pattern in ["pharmacy", "cvs", "walgreens", "rite aid", "doctor", "physician", "medical", "clinic", "hospital", "dental", "dentist", "optometrist", "vision", "therapy", "mental health", "chiropractor", "dermatology", "urgent care", "labcorp", "quest diagnostics", "zocdoc"] {
        rules.push(CategoryRule::new("cat_expense_healthcare".into(), pattern.into(), "description".into()));
    }

    // Shopping (general retailers - Walmart is a catch-all since it sells everything)
    for pattern in [
        "amazon.com", "amazon purchase", "amzn", "walmart", "wal-mart", "target", "best buy",
        "home depot", "lowe's", "costco", "ikea", "bed bath", "wayfair", "overstock",
        "etsy", "ebay", "aliexpress", "wish.com", "macy's", "nordstrom", "kohl's",
        "jc penney", "sears", "marshalls", "tj maxx", "ross", "burlington", "dollar",
        "big lots", "michaels", "joann", "hobby lobby", "five below", "dollar tree",
        "dollar general", "family dollar", "menards", "ace hardware", "tractor supply",
    ] {
        rules.push(CategoryRule::new("cat_expense_shopping".into(), pattern.into(), "description".into()));
    }

    // Entertainment
    for pattern in ["movie", "cinema", "theater", "theatre", "concert", "ticketmaster", "stubhub", "seatgeek", "eventbrite", "amc", "regal", "cinemark", "bowling", "arcade", "amusement", "theme park", "zoo", "aquarium", "museum", "escape room", "laser tag", "mini golf", "golf course", "gym membership", "fitness", "planet fitness", "la fitness", "equinox", "orangetheory", "crossfit", "peloton", "classpass"] {
        rules.push(CategoryRule::new("cat_expense_entertainment".into(), pattern.into(), "description".into()));
    }

    // Personal Care
    for pattern in ["salon", "barber", "haircut", "spa", "massage", "manicure", "pedicure", "nail salon", "waxing", "ulta", "sephora", "beauty", "cosmetic", "skincare"] {
        rules.push(CategoryRule::new("cat_expense_personal".into(), pattern.into(), "description".into()));
    }

    // Education
    for pattern in ["tuition", "university", "college", "school fee", "coursera", "udemy", "linkedin learning", "skillshare", "masterclass", "duolingo", "rosetta stone", "chegg", "textbook", "education"] {
        rules.push(CategoryRule::new("cat_expense_education".into(), pattern.into(), "description".into()));
    }

    // Travel
    for pattern in ["airline", "delta", "united airlines", "american airlines", "southwest", "jetblue", "spirit", "frontier", "alaska air", "hotel", "marriott", "hilton", "hyatt", "ihg", "wyndham", "best western", "airbnb", "vrbo", "booking.com", "expedia", "kayak", "priceline", "hotwire", "trivago", "rental car", "cruise"] {
        rules.push(CategoryRule::new("cat_expense_travel".into(), pattern.into(), "description".into()));
    }

    // Pets
    for pattern in ["petco", "petsmart", "pet supplies", "veterinary", "vet clinic", "dog food", "cat food", "pet food", "chewy.com", "rover", "wag walking", "pet boarding"] {
        rules.push(CategoryRule::new("cat_expense_pets".into(), pattern.into(), "description".into()));
    }

    // Gifts & Donations
    for pattern in ["donation", "charity", "gofundme", "patreon", "kickstarter", "indiegogo", "red cross", "salvation army", "goodwill", "united way", "nonprofit"] {
        rules.push(CategoryRule::new("cat_expense_gifts".into(), pattern.into(), "description".into()));
    }

    // Fees & Charges
    for pattern in ["atm fee", "overdraft", "service charge", "monthly fee", "maintenance fee", "wire transfer", "foreign transaction", "late fee", "interest charge", "finance charge", "annual fee"] {
        rules.push(CategoryRule::new("cat_expense_fees".into(), pattern.into(), "description".into()));
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TransactionType;
    use chrono::Utc;
    use rust_decimal::Decimal;

    #[test]
    fn test_categorizer_matches_pattern() {
        let rules = vec![
            CategoryRule::new("cat_groceries".into(), "grocery".into(), "description".into()),
        ];

        let categorizer = Categorizer::new(rules);

        let tx = Transaction::new(
            "acc1".into(),
            TransactionType::Expense,
            Decimal::new(5000, 2),
            Utc::now().date_naive(),
            "GROCERY STORE #123".into(),
        );

        assert_eq!(categorizer.categorize(&tx), Some("cat_groceries".into()));
    }
}
