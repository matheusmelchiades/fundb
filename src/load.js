const Fs = require('fs');

const Config = require('../config');

function load () {

  const database = {};

  const { path } = Config;

  const files = Fs.readdirSync(path);

  const tables = files.map(file => {

    const names = file.split('.');

    const filePath = path + '/' + file;

    const content = Fs.readFileSync(filePath);

    names.pop();

    return {
      name: names.join(),
      path: filePath,
      data: JSON.parse(content),
    };
    
  });

  tables.forEach(table => database[table.name] = table);

  global.database = database;

}

module.exports = load;