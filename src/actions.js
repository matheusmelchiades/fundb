const fs = require('fs');

const sgdb = require('./sgdb');
const config = require('../config');
const types = require('./types')

const fundb = new sgdb(config.path);

exports.createTable = ([ table ]) => {

    if (!table)
        return console.log('Input invalid!')

    fundb.createTable(table)

}

exports.findByIndex = ([ table, index ]) => {

    const data = global.database[table].data;
    
    const object = data[index -1] || null;

    console.log(' Result: \n')
    console.log(object)
    console.log()
}

exports.insert = ([ tableName, codigo, descricao ]) => {

    const obj = { codigo, descricao }

    const table = global.database[tableName];

    table.data.push(obj);

    fs.writeFileSync(table.path, JSON.stringify(table.data));

    global.database[tableName].data = table.data;

}

exports.setDatabase = ([ dbName ], rl) => {

    console.log(fundb)

    const db = fundb.setCurrentDatabase(dbName)

    if (!db)
        console.log('Database not exists')

    rl.setPrompt(`🛢\  >> ${db} 👉 `)

}

exports.listTables = () => {

    const tables = Object.keys(global.database);

    console.log('\n');
    tables.map(table => console.log(`  📂\ >> ${table}`))
    console.log('\n');

}

exports.help = () => {

    console.log('\n')

    for (const action in types) {

        console.log(types[action].help, '\n')

    }

}