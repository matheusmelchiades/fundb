const readline = require('readline');
const prompt = '🚀 FunDB  👉 '
const actions = require('./actions')

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt
});

const { new_db, find, help, insert, new_table, show_dbs, show_tables, use } = require('./types')

console.log("\nYou're welcome to FunDB!\n")

rl.prompt('\n');

rl.on('line', (line) => {

    try {

        const commands = line.split(' ')

        const command = commands.shift().trim()

        switch (command) {
            case new_db.label:
                actions.createDB(commands)
                break;

            case new_table.label:
                actions.createTable(commands)
                break;

            case show_dbs.label:
                actions.listDatabases()
                break;

            case show_tables.label:
                actions.listTables()
                break;

            case find.label:
                actions.findByIndex(commands)
                break;
                
            case insert.label:
                actions.insert(commands)
                break;

            case use.label:
                actions.setDatabase(commands, rl)
                break;

            case help.label:
                actions.help(commands, rl)
                break;

            default:
                console.log('\nCommand invalid!\n')
                break;
        }

        rl.prompt('\n');

    } catch(error) {
        console.error(error.message);
    }

})

rl.on('close', () => {
    console.log('\n\n FUNDB Is THE BEST MOTHER FUCK!\n\n');
    process.exit(0);
});