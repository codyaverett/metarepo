use colored::*;

pub fn print_full_documentation() {
    println!("{}", "═══════════════════════════════════════════════════════════════".blue().bold());
    println!("{}", "           GESTALT RULES - COMPLETE DOCUMENTATION".cyan().bold());
    println!("{}", "═══════════════════════════════════════════════════════════════".blue().bold());
    println!();
    
    print_overview();
    print_rule_types();
    print_configuration_format();
    print_examples();
    print_best_practices();
}

pub fn print_directory_rule_docs() {
    println!("{}", "DIRECTORY RULES".cyan().bold());
    println!("{}", "═══════════════".blue());
    println!();
    println!("Directory rules ensure specific directories exist in your projects.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("directories:");
    println!("  - path: src           # Path relative to project root");
    println!("    required: true      # true = error if missing, false = info");
    println!("    description: Source code directory");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Directory path relative to project root", "path".green());
    println!("  • {}: Whether the directory must exist", "required".green());
    println!("  • {}: Human-readable description", "description".green());
    println!();
    println!("{}", "Auto-fix:".yellow());
    println!("  ✅ Missing directories can be automatically created with --fix");
}

pub fn print_component_rule_docs() {
    println!("{}", "COMPONENT RULES".cyan().bold());
    println!("{}", "═══════════════".blue());
    println!();
    println!("Component rules validate folder structures for components matching a pattern.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("components:");
    println!("  - pattern: components/**/  # Glob pattern for component dirs");
    println!("    structure:               # Required structure within");
    println!("      - '[ComponentName].vue'");
    println!("      - '__tests__/'");
    println!("      - '__tests__/[ComponentName].test.js'");
    println!("    description: Vue component structure");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Glob pattern to match component directories", "pattern".green());
    println!("  • {}: List of required files/directories", "structure".green());
    println!("  • {}: Human-readable description", "description".green());
    println!();
    println!("{}", "Placeholders:".yellow());
    println!("  • {} is replaced with the actual component name", "[ComponentName]".cyan());
    println!();
    println!("{}", "Auto-fix:".yellow());
    println!("  ✅ Missing directories in structure can be created");
    println!("  ❌ Missing files must be created manually");
}

pub fn print_file_rule_docs() {
    println!("{}", "FILE RULES".cyan().bold());
    println!("{}", "══════════".blue());
    println!();
    println!("File rules ensure files matching a pattern have required companions.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("files:");
    println!("  - pattern: '**/*.vue'      # Files to check");
    println!("    requires:                # Required companion files");
    println!("      test: '__tests__/*.test.js'");
    println!("      story: '*.stories.js'");
    println!("    description: Vue files must have tests and stories");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Glob pattern for files to check", "pattern".green());
    println!("  • {}: Map of required file types and their patterns", "requires".green());
    println!("  • {}: Human-readable description", "description".green());
    println!();
    println!("{}", "Special Patterns:".yellow());
    println!("  • {}: Looks for test annotations within the file itself", "#[test]".cyan());
    println!("  • {}: Replaced with the base filename", "*".cyan());
    println!();
    println!("{}", "Auto-fix:".yellow());
    println!("  ❌ Companion files must be created manually");
}

fn print_overview() {
    println!("{}", "OVERVIEW".cyan().bold());
    println!("{}", "════════".blue());
    println!();
    println!("The Rules plugin enforces consistent project structure across your workspace.");
    println!("It validates directories, component structures, and file dependencies.");
    println!();
    println!("{}", "Key Features:".yellow());
    println!("  • Three rule types: directories, components, and files");
    println!("  • YAML/JSON configuration support");
    println!("  • Project-specific and workspace-wide rules");
    println!("  • Auto-fix capabilities for missing directories");
    println!("  • Integration with AI assistants for context building");
    println!();
}

fn print_rule_types() {
    println!("{}", "RULE TYPES".cyan().bold());
    println!("{}", "══════════".blue());
    println!();
    println!("1. {} - Ensure directories exist", "Directory Rules".green());
    println!("2. {} - Validate component folder structures", "Component Rules".green());
    println!("3. {} - Check for required companion files", "File Rules".green());
    println!();
}

fn print_configuration_format() {
    println!("{}", "CONFIGURATION FORMAT".cyan().bold());
    println!("{}", "════════════════════".blue());
    println!();
    println!("Rules can be defined in multiple locations:");
    println!();
    println!("1. {} - Workspace-wide rules", ".rules.yaml".green());
    println!("2. {} - Project-specific rules", "<project>/.rules.yaml".green());
    println!("3. {} - Project rules in meta config", ".meta (rules section)".green());
    println!();
    println!("{}", "Priority (highest to lowest):".yellow());
    println!("  1. Project-specific .rules.yaml");
    println!("  2. Project rules in .meta");
    println!("  3. Workspace .rules.yaml");
    println!();
}

fn print_examples() {
    println!("{}", "EXAMPLES".cyan().bold());
    println!("{}", "════════".blue());
    println!();
    
    println!("{}", "Vue.js Project:".yellow());
    println!("```yaml");
    println!("directories:");
    println!("  - { path: src/components, required: true }");
    println!("  - { path: tests, required: true }");
    println!();
    println!("components:");
    println!("  - pattern: 'src/components/**/'");
    println!("    structure:");
    println!("      - '[ComponentName].vue'");
    println!("      - '[ComponentName].test.js'");
    println!("      - '[ComponentName].stories.js'");
    println!("```");
    println!();
    
    println!("{}", "React TypeScript Project:".yellow());
    println!("```yaml");
    println!("components:");
    println!("  - pattern: 'src/components/**/'");
    println!("    structure:");
    println!("      - '[ComponentName].tsx'");
    println!("      - '[ComponentName].test.tsx'");
    println!("      - '[ComponentName].module.css'");
    println!("      - 'index.ts'");
    println!("```");
    println!();
    
    println!("{}", "Rust Project:".yellow());
    println!("```yaml");
    println!("directories:");
    println!("  - { path: src, required: true }");
    println!("  - { path: benches, required: false }");
    println!();
    println!("files:");
    println!("  - pattern: 'src/**/*.rs'");
    println!("    requires:");
    println!("      test: '#[test]'  # Looks for test annotations");
    println!("```");
    println!();
}

fn print_best_practices() {
    println!("{}", "BEST PRACTICES".cyan().bold());
    println!("{}", "══════════════".blue());
    println!();
    println!("1. {} - Define common rules at workspace level", "Start Simple".green());
    println!("2. {} - Override with project-specific rules as needed", "Be Specific".green());
    println!("3. {} - Mark optional directories as required: false", "Use Severity".green());
    println!("4. {} - Add descriptions for team understanding", "Document Rules".green());
    println!("5. {} - Use --fix during development", "Automate".green());
    println!("6. {} - Add rules check to CI/CD pipeline", "Enforce".green());
    println!();
    println!("{}", "AI Assistant Integration:".yellow());
    println!("• Run 'gest rules check' before making structural changes");
    println!("• Use 'gest rules docs' to understand project conventions");
    println!("• Apply '--fix' to quickly scaffold required structure");
    println!();
}

pub fn print_create_help() {
    println!("{}", "CREATING RULES".cyan().bold());
    println!("{}", "══════════════".blue());
    println!();
    println!("Use the create subcommands to add new rules:");
    println!();
    println!("{}", "Available Commands:".yellow());
    println!("  {} - Add a directory rule", "gest rules create directory <path>".green());
    println!("  {} - Add a component rule", "gest rules create component <pattern>".green());
    println!("  {} - Add a file rule", "gest rules create file <pattern>".green());
    println!();
    println!("{}", "Options:".yellow());
    println!("  {} - Target specific project", "--project <name>".cyan());
    println!("  {} - Mark as required (directory rules)", "--required".cyan());
    println!("  {} - Add description", "--description <text>".cyan());
    println!();
    println!("{}", "Examples:".yellow());
    println!("  gest rules create directory src/utils --required");
    println!("  gest rules create component 'components/**/' --project frontend");
    println!("  gest rules create file '**/*.ts' --description 'TypeScript files'");
    println!();
}