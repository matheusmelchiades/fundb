const config = require('../config');
const types = require('./types')
const sgdb = require('./sgdb');
const fundb = new sgdb(config.path);

const noTable = () => console.log('\nTable not selected!\n');

exports.createTable = ([table]) => {

    if (!table)
        return console.log('Input invalid!')

    fundb.createTable(table)

}

exports.findByIndex = ([table, index]) => {

    const data = global.database[table].data;

    const object = data[index - 1] || null;

    console.log(' Result: \n')
    console.log(object)
    console.log()
}

exports.find = ([]) => {
    if (!fundb.getCurrentTable())
        return noTable()

    const data = fundb.findAll()

    console.log('\n')
    console.table(data[0], data)
    console.log('\n')
    return
}

exports.insert = ([codigo, descricao]) => {

    if (!fundb.getCurrentTable())
        return noTable()

    fundb.insert({ codigo, descricao });
}

exports.setTable = ([name]) => {

    const table = fundb.setCurrentTable(name)

    if (!table)
        console.log('\nTable not exists\n')

    config.rl.setPrompt(`${config.samplePrompt} 📂\  ${table} 👉 `)
}

exports.listTables = () => {

    const tables = fundb.listTables();

    if (!tables.length)
        return console.log('\nNo exists tables!\n')

    console.log('\n')
    tables.map(table => console.log(`  📂\ >> ${table}`))
    console.log('\n')

}

exports.help = () => {

    console.log('\n')

    for (const action in types) {

        console.log(types[action].help, '\n')

    }

}