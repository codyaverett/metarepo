use colored::*;

pub fn print_full_documentation() {
    print_full_documentation_formatted(false);
}

pub fn print_full_documentation_ai() {
    print_full_documentation_formatted(true);
}

fn print_full_documentation_formatted(ai_mode: bool) {
    if ai_mode {
        print_ai_optimized_docs();
    } else {
        println!(
            "{}",
            "═══════════════════════════════════════════════════════════════"
                .blue()
                .bold()
        );
        println!(
            "{}",
            "           GESTALT RULES - COMPLETE DOCUMENTATION"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "═══════════════════════════════════════════════════════════════"
                .blue()
                .bold()
        );
        println!();

        print_overview();
        print_rule_types();
        print_configuration_format();
        print_examples();
        print_best_practices();
    }
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
    println!(
        "  • {}: Directory path relative to project root",
        "path".green()
    );
    println!(
        "  • {}: Whether the directory must exist",
        "required".green()
    );
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
    println!(
        "  • {}: Glob pattern to match component directories",
        "pattern".green()
    );
    println!(
        "  • {}: List of required files/directories",
        "structure".green()
    );
    println!("  • {}: Human-readable description", "description".green());
    println!();
    println!("{}", "Placeholders:".yellow());
    println!(
        "  • {} is replaced with the actual component name",
        "[ComponentName]".cyan()
    );
    println!();
    println!("{}", "Auto-fix:".yellow());
    println!("  ✅ Missing directories in structure can be created");
    println!("  ❌ Missing files must be created manually");
}

pub fn print_naming_rule_docs() {
    println!("{}", "NAMING RULES".cyan().bold());
    println!("{}", "════════════".blue());
    println!();
    println!("Naming rules enforce consistent file and directory naming conventions.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("naming:");
    println!("  - pattern: 'src/components/**/*.tsx'");
    println!("    naming_pattern: '^[A-Z][a-zA-Z0-9]+\\.tsx$'");
    println!("    case_style: PascalCase  # Optional hint");
    println!("    description: React components must be PascalCase");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Glob pattern for files to check", "pattern".green());
    println!(
        "  • {}: Regex pattern for valid names",
        "naming_pattern".green()
    );
    println!("  • {}: Optional naming style hint", "case_style".green());
    println!("    Options: PascalCase, camelCase, snake_case, UPPER_CASE, kebab-case");
    println!("  • {}: Human-readable description", "description".green());
    println!();
    println!("{}", "Examples:".yellow());
    println!("  • React hooks: pattern: 'hooks/*.ts', naming: '^use[A-Z].*'");
    println!("  • Constants: pattern: 'constants/*.ts', case_style: 'UPPER_CASE'");
    println!("  • CSS modules: pattern: '*.module.css', case_style: 'kebab-case'");
}

pub fn print_dependency_rule_docs() {
    println!("{}", "DEPENDENCY RULES".cyan().bold());
    println!("{}", "════════════════".blue());
    println!();
    println!("Dependency rules control which packages can be used in your projects.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("dependencies:");
    println!("  - forbidden:");
    println!("      - lodash        # Use native methods instead");
    println!("      - moment        # Use date-fns instead");
    println!("    required:");
    println!("      react: '^18.0.0'");
    println!("      typescript: '^5.0.0'");
    println!("    description: Package constraints");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!(
        "  • {}: List of packages that must not be used",
        "forbidden".green()
    );
    println!(
        "  • {}: Map of required packages and versions",
        "required".green()
    );
    println!("  • {}: Maximum dependency depth", "max_depth".green());
    println!("  • {}: Human-readable description", "description".green());
    println!();
    println!("{}", "Supported Files:".yellow());
    println!("  • package.json (Node.js projects)");
    println!("  • Cargo.toml (Rust projects)");
}

pub fn print_import_rule_docs() {
    println!("{}", "IMPORT RULES".cyan().bold());
    println!("{}", "════════════".blue());
    println!();
    println!("Import rules control module boundaries and import patterns.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("imports:");
    println!("  - source_pattern: 'src/components/**/*.tsx'");
    println!("    forbidden_imports:");
    println!("      - '../../../utils'  # No deep relative imports");
    println!("      - 'src/internal'    # Internal modules");
    println!("    require_absolute: true");
    println!("    description: Component import constraints");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Files to check", "source_pattern".green());
    println!(
        "  • {}: List of allowed import patterns",
        "allowed_imports".green()
    );
    println!(
        "  • {}: List of forbidden import patterns",
        "forbidden_imports".green()
    );
    println!(
        "  • {}: Require absolute over relative imports",
        "require_absolute".green()
    );
    println!("  • {}: Maximum import depth", "max_depth".green());
}

pub fn print_documentation_rule_docs() {
    println!("{}", "DOCUMENTATION RULES".cyan().bold());
    println!("{}", "═══════════════════".blue());
    println!();
    println!("Documentation rules ensure proper documentation coverage.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("documentation:");
    println!("  - pattern: 'src/**/*.ts'");
    println!("    require_header: true");
    println!("    require_examples: true");
    println!("    required_sections:");
    println!("      - Usage");
    println!("      - Parameters");
    println!("      - Returns");
    println!("    description: TypeScript documentation requirements");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Files to check", "pattern".green());
    println!(
        "  • {}: Require file header comments",
        "require_header".green()
    );
    println!("  • {}: Require code examples", "require_examples".green());
    println!(
        "  • {}: Minimum description length",
        "min_description_length".green()
    );
    println!(
        "  • {}: Required documentation sections",
        "required_sections".green()
    );
}

pub fn print_size_rule_docs() {
    println!("{}", "SIZE RULES".cyan().bold());
    println!("{}", "══════════".blue());
    println!();
    println!("Size rules control file complexity and size limits.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("size:");
    println!("  - pattern: '**/*.js'");
    println!("    max_lines: 500");
    println!("    max_bytes: 50000");
    println!("    max_functions: 10");
    println!("    description: JavaScript file size limits");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Files to check", "pattern".green());
    println!("  • {}: Maximum line count", "max_lines".green());
    println!("  • {}: Maximum file size in bytes", "max_bytes".green());
    println!(
        "  • {}: Maximum number of functions",
        "max_functions".green()
    );
    println!(
        "  • {}: Maximum cyclomatic complexity",
        "max_complexity".green()
    );
}

pub fn print_security_rule_docs() {
    println!("{}", "SECURITY RULES".cyan().bold());
    println!("{}", "══════════════".blue());
    println!();
    println!("Security rules check for common security issues in your code.");
    println!();
    println!("{}", "Configuration:".yellow());
    println!("```yaml");
    println!("security:");
    println!("  - pattern: '**/*.{{js,ts,py}}'");
    println!("    forbidden_patterns:");
    println!("      - 'api[_-]?key.*=.*[\"\\']'  # No hardcoded API keys");
    println!("      - 'password.*=.*[\"\\']'      # No hardcoded passwords");
    println!("    forbidden_functions:");
    println!("      - eval");
    println!("      - exec");
    println!("    require_https: true");
    println!("    description: Basic security checks");
    println!("```");
    println!();
    println!("{}", "Properties:".yellow());
    println!("  • {}: Glob pattern for files to check", "pattern".green());
    println!(
        "  • {}: Regex patterns to flag",
        "forbidden_patterns".green()
    );
    println!(
        "  • {}: Functions that shouldn't be used",
        "forbidden_functions".green()
    );
    println!("  • {}: Flag non-HTTPS URLs", "require_https".green());
    println!(
        "  • {}: Check for hardcoded secrets",
        "no_hardcoded_secrets".green()
    );
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
    println!(
        "  • {}: Map of required file types and their patterns",
        "requires".green()
    );
    println!("  • {}: Human-readable description", "description".green());
    println!();
    println!("{}", "Special Patterns:".yellow());
    println!(
        "  • {}: Looks for test annotations within the file itself",
        "#[test]".cyan()
    );
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
    println!("It validates directories, component structures, file dependencies, naming");
    println!("conventions, security standards, and more.");
    println!();
    println!("{}", "Key Features:".yellow());
    println!("  • Nine rule types for comprehensive validation");
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
    println!("{}", "Structure Rules:".yellow());
    println!(
        "  1. {} - Ensure directories exist",
        "Directory Rules".green()
    );
    println!(
        "  2. {} - Validate component folder structures",
        "Component Rules".green()
    );
    println!(
        "  3. {} - Check for required companion files",
        "File Rules".green()
    );
    println!();
    println!("{}", "Quality Rules:".yellow());
    println!(
        "  4. {} - Enforce file naming conventions",
        "Naming Rules".green()
    );
    println!(
        "  5. {} - Control file size and complexity",
        "Size Rules".green()
    );
    println!(
        "  6. {} - Ensure documentation coverage",
        "Documentation Rules".green()
    );
    println!();
    println!("{}", "Architecture Rules:".yellow());
    println!(
        "  7. {} - Manage allowed/forbidden packages",
        "Dependency Rules".green()
    );
    println!("  8. {} - Control import patterns", "Import Rules".green());
    println!("  9. {} - Basic security checks", "Security Rules".green());
    println!();
}

fn print_configuration_format() {
    println!("{}", "CONFIGURATION FORMAT".cyan().bold());
    println!("{}", "════════════════════".blue());
    println!();
    println!("Rules can be defined in multiple locations:");
    println!();
    println!("1. {} - Workspace-wide rules", ".rules.yaml".green());
    println!(
        "2. {} - Project-specific rules",
        "<project>/.rules.yaml".green()
    );
    println!(
        "3. {} - Project rules in meta config",
        ".meta (rules section)".green()
    );
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
    println!("  - {{ path: src/components, required: true }}");
    println!("  - {{ path: tests, required: true }}");
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
    println!("  - {{ path: src, required: true }}");
    println!("  - {{ path: benches, required: false }}");
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
    println!(
        "1. {} - Define common rules at workspace level",
        "Start Simple".green()
    );
    println!(
        "2. {} - Override with project-specific rules as needed",
        "Be Specific".green()
    );
    println!(
        "3. {} - Mark optional directories as required: false",
        "Use Severity".green()
    );
    println!(
        "4. {} - Add descriptions for team understanding",
        "Document Rules".green()
    );
    println!("5. {} - Use --fix during development", "Automate".green());
    println!(
        "6. {} - Add rules check to CI/CD pipeline",
        "Enforce".green()
    );
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
    println!(
        "  {} - Add a directory rule",
        "gest rules create directory <path>".green()
    );
    println!(
        "  {} - Add a component rule",
        "gest rules create component <pattern>".green()
    );
    println!(
        "  {} - Add a file rule",
        "gest rules create file <pattern>".green()
    );
    println!();
    println!("{}", "Options:".yellow());
    println!("  {} - Target specific project", "--project <name>".cyan());
    println!(
        "  {} - Mark as required (directory rules)",
        "--required".cyan()
    );
    println!("  {} - Add description", "--description <text>".cyan());
    println!();
    println!("{}", "Examples:".yellow());
    println!("  gest rules create directory src/utils --required");
    println!("  gest rules create component 'components/**/' --project frontend");
    println!("  gest rules create file '**/*.ts' --description 'TypeScript files'");
    println!();
}

fn print_ai_optimized_docs() {
    println!("# Gestalt Rules Plugin");
    println!();
    println!("## Available Rule Types");
    println!();
    println!("### Structure Rules");
    println!("- **directories**: Ensure specific directories exist (auto-fixable)");
    println!("- **components**: Validate component folder structures");
    println!("- **files**: Check for required companion files (tests, stories, etc.)");
    println!();
    println!("### Quality Rules");
    println!("- **naming**: Enforce file naming conventions (PascalCase, camelCase, etc.)");
    println!("- **size**: Control file size limits (lines, bytes, functions)");
    println!("- **documentation**: Ensure documentation coverage");
    println!();
    println!("### Architecture Rules");
    println!("- **dependencies**: Control allowed/forbidden packages");
    println!("- **imports**: Manage import patterns and module boundaries");
    println!("- **security**: Basic security checks (no hardcoded secrets, dangerous functions)");
    println!();
    println!("## Configuration Schema");
    println!();
    println!("```yaml");
    println!("# All fields are optional and default to empty arrays");
    println!("directories:");
    println!("  - path: string");
    println!("    required: boolean");
    println!("    description: string");
    println!();
    println!("components:");
    println!("  - pattern: string  # glob pattern");
    println!("    structure: [string]  # [ComponentName] placeholder");
    println!();
    println!("files:");
    println!("  - pattern: string");
    println!("    requires: {{type: pattern}}");
    println!();
    println!("naming:");
    println!("  - pattern: string");
    println!("    naming_pattern: string  # regex");
    println!("    case_style: string  # PascalCase|camelCase|snake_case|UPPER_CASE|kebab-case");
    println!();
    println!("dependencies:");
    println!("  - forbidden: [string]");
    println!("    required: {{package: version}}");
    println!("    max_depth: number");
    println!();
    println!("imports:");
    println!("  - source_pattern: string");
    println!("    allowed_imports: [string]");
    println!("    forbidden_imports: [string]");
    println!("    require_absolute: boolean");
    println!();
    println!("documentation:");
    println!("  - pattern: string");
    println!("    require_header: boolean");
    println!("    require_examples: boolean");
    println!("    required_sections: [string]");
    println!();
    println!("size:");
    println!("  - pattern: string");
    println!("    max_lines: number");
    println!("    max_bytes: number");
    println!("    max_functions: number");
    println!();
    println!("security:");
    println!("  - pattern: string");
    println!("    forbidden_patterns: [string]  # regex");
    println!("    forbidden_functions: [string]");
    println!("    require_https: boolean");
    println!("```");
    println!();
    println!("## Severity Levels");
    println!("- **Error**: Required rules that must be fixed");
    println!("- **Warning**: Recommended rules that should be addressed");
    println!("- **Info**: Optional rules for awareness");
    println!();
    println!("## Auto-fix Capabilities");
    println!("- ✅ Directory creation");
    println!("- ✅ Component directory structure");
    println!("- ❌ File content (must be created manually)");
    println!();
    println!("## Configuration Precedence");
    println!("1. Project-specific `.rules.yaml`");
    println!("2. Workspace `.rules.yaml`");
    println!("3. Default minimal rules");
}
