import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';
StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  const color = await i.select('What is your favorite color?', ['red', 'blue', 'green'], 'cyane2e/template1/color');
  const name = await i.text('What is your name?', 'cyane2e/template1/name');
  const food = await i.select('What is your favorite food?', ['pizza', 'burger', 'salad'], 'cyane2e/template1/food');
  const country = await i.select(
    'What is your favorite country?',
    ['USA', 'Canada', 'Mexico'],
    'cyane2e/template1/country',
  );
  const animal = await i.select('What is your favorite animal?', ['dog', 'cat', 'bird'], 'cyane2e/template1/animal');
  const sport = await i.select(
    'What is your favorite sport?',
    ['soccer', 'basketball', 'tennis'],
    'cyane2e/template1/sport',
  );
  const season = await i.select(
    'What is your favorite season?',
    ['spring', 'summer', 'fall', 'winter'],
    'cyane2e/template1/season',
  );
  const music = await i.select(
    'What is your favorite music genre?',
    ['rock', 'pop', 'jazz', 'classical'],
    'cyane2e/template1/music',
  );
  const hobby = await i.select(
    'What is your favorite hobby?',
    ['reading', 'gaming', 'cooking', 'hiking'],
    'cyane2e/template1/hobby',
  );

  return {
    processors: [
      {
        name: 'cyane2e/processor1',
        files: [
          {
            glob: '**/*.*',
            exclude: [],
            type: GlobType.Template,
            root: 'template',
          },
        ],
        config: {
          vars: {
            color,
            name,
            food,
            country,
            animal,
            sport,
            season,
            music,
            hobby,
          },
        },
      },
    ],
    plugins: [
      {
        name: 'cyane2e/plugin1',
        config: {},
      },
    ],
  };
});
