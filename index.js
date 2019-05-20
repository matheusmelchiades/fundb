const readline = require('readline');
const prompt = '🚀 FunDB  👉 '
const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt
});
const actions = require('./actions')

console.log("\nYou're welcome to FunDB!\n")

rl.prompt('\n');

rl.on('line', (line) => {
    const commands = line.split(' ')

    switch (commands[0].trim()) {
        case 'new_db':
            actions.createDB(commands[1])
            break;
        case 'new_table':
            actions.createTable(commands[1])
            break;
        case 'show_dbs':
            actions.listDatabases()
            break;
        case 'show_tables':
            actions.listTables()
            break;

        case 'find':
            actions.findByIndex(commands[1], commands[2])
            break;
            
        case 'insert':
            const obj = {
                name: commands[2],
                description: commands[3]
            }
            actions.insert(commands[1], obj)
            break;

        case 'use':
            const db = actions.setDatabase(commands[1])
            if (!db) break

            rl.setPrompt(`🛢\  >> ${db} 👉 `)
            break;
        default:
            console.log('\nCommand invalid!\n')
            break;
    }

    rl.prompt('\n');

}).on('close', () => {
    console.log('\n\n FUNDB Is THE BEST MOTHER FUCK!\n\n');
    process.exit(0);
});